//! Lane H: bracket dragging publishes one checked command on release.
use super::*;
use varos_app::shell::tokens as t;
use varos_core::typography::{Action, Binding};
use varos_text_layout::flow::ArcPath;
pub(super) struct Drag {
    pub id: u32,
    pub binding: Binding,
    pub arc: ArcPath,
    pub start: bool,
}
fn arc(ed: &Editor, id: u32, binding: Binding) -> Option<ArcPath> {
    let Binding::Path { path, .. } = binding else {
        return None;
    };
    let i = ed.doc.pidx(path)?;
    let text = ed.doc.text_boxes.iter().find(|t| t.id == id)?;
    let xf = varos_core::typography::text_transform(&ed.doc, id);
    let points = varos_text_layout::flow::geometry(&ed.doc, text, path).ok()?.into_iter().next()?;
    ArcPath::new(points.into_iter().map(|p| xf.apply(p)).collect(), ed.doc.paths[i].closed).ok()
}
impl TextProduct {
    pub(super) fn bracket_event(
        &mut self,
        ctx: &egui::Context,
        event: &Event,
        ed: &mut Editor,
        view: View,
        ppp: f32,
        hole: Option<egui::Rect>,
    ) -> Result<bool, String> {
        match event {
            Event::PointerButton { pos, pressed: true, button: egui::PointerButton::Primary, .. }
                if self.path_drag.is_none() =>
            {
                if !hole.is_some_and(|h| h.contains(*pos))
                    || ctx.layer_id_at(*pos).is_some_and(|l| l.order != egui::Order::Background)
                {
                    return Ok(false);
                }
                let Some(id) = self.selected else {
                    return Ok(false);
                };
                let Some(binding) = ed.doc.typography.frames.get(&id).and_then(|f| f.binding) else {
                    return Ok(false);
                };
                let Binding::Path { start, end, .. } = binding else {
                    return Ok(false);
                };
                let Some(arc) = arc(ed, id, binding) else {
                    return Ok(false);
                };
                let p = view.s2w([pos.x * ppp, pos.y * ppp]);
                for (is_start, d) in [(true, start), (false, arc.length - end)] {
                    if arc
                        .sample(d)
                        .is_some_and(|(q, _)| varos_core::geom::dist(q, p) * view.zoom / ppp <= t::TEXT_PATH_HIT_RADIUS)
                    {
                        self.commit(ed)?;
                        self.path_drag = Some(Drag { id, binding, arc, start: is_start });
                        return Ok(true);
                    }
                }
            }
            Event::PointerMoved(pos) if self.path_drag.is_some() => {
                if let Some(drag) = &mut self.path_drag {
                    let distance = drag.arc.nearest(view.s2w([pos.x * ppp, pos.y * ppp]));
                    if let Binding::Path { start, end, .. } = &mut drag.binding {
                        if drag.start {
                            *start = distance.min(drag.arc.length - *end).max(0.);
                        } else {
                            *end = (drag.arc.length - distance).min(drag.arc.length - *start).max(0.);
                        }
                    }
                }
                return Ok(true);
            }
            Event::PointerButton { pressed: false, button: egui::PointerButton::Primary, .. }
                if self.path_drag.is_some() =>
            {
                if let Some(drag) = self.path_drag.take() {
                    ed.try_execute(EditCommand::Typography(Action::Bind {
                        text: drag.id,
                        binding: Some(drag.binding),
                    }))?;
                    self.generation += 1;
                }
                return Ok(true);
            }
            Event::Key { key: Key::Escape, pressed: true, .. } if self.path_drag.is_some() => {
                self.path_drag = None;
                return Ok(true);
            }
            _ => {}
        }
        Ok(false)
    }
    pub(super) fn paint_brackets(&self, ctx: &egui::Context, ed: &Editor, view: View, ppp: f32) {
        let Some(id) = self.selected else {
            return;
        };
        let Some(binding) = self
            .path_drag
            .as_ref()
            .map(|d| d.binding)
            .or_else(|| ed.doc.typography.frames.get(&id).and_then(|f| f.binding))
        else {
            return;
        };
        let Binding::Path { start, end, .. } = binding else {
            return;
        };
        let Some(arc) = arc(ed, id, binding) else {
            return;
        };
        let painter =
            ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("text-path-brackets")));
        for distance in [start, arc.length - end] {
            if let Some((point, tangent)) = arc.sample(distance) {
                let p = view.w2s(point);
                let p = egui::pos2(p[0] / ppp, p[1] / ppp);
                let normal = egui::vec2(-tangent[1], tangent[0]) * t::TEXT_PATH_BRACKET_HALF;
                painter.line_segment([p - normal, p + normal], egui::Stroke::new(t::TEXT_CARET_W, t::ACCENT));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bracket_drag_is_one_undoable_command_and_escape_cancels() {
        let mut ed = Editor::new();
        let path = ed.doc.nid();
        let anchors = [[0., 100.], [300., 100.]]
            .into_iter()
            .map(|p| varos_core::model::Anchor { id: ed.doc.nid(), p, hin: None, hout: None, smooth: false })
            .collect();
        ed.doc.paths.push(varos_core::model::Path::new(path, anchors, false, None, None, 0.));
        ed.doc.sync_tree();
        let id = ed
            .try_execute_created(EditCommand::AddText {
                text: varos_text_layout::default_text("سلام", [0., 100.]).unwrap(),
                parent: None,
            })
            .unwrap();
        let binding = Binding::Path {
            path,
            start: 30.,
            end: 0.,
            offset: 0.,
            flip: false,
            effect: varos_core::typography::PathEffect::Rainbow,
        };
        ed.try_execute(EditCommand::Typography(Action::Bind { text: id, binding: Some(binding) })).unwrap();
        let mut tool = TextProduct { selected: Some(id), ..Default::default() };
        let ctx = egui::Context::default();
        let view = View::identity();
        let hole = Some(egui::Rect::from_min_max(egui::pos2(0., 0.), egui::pos2(500., 500.)));
        let button = |x, pressed| Event::PointerButton {
            pos: egui::pos2(x, 100.),
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        let rev = ed.rev;
        for e in [button(30., true), Event::PointerMoved(egui::pos2(60., 100.)), button(60., false)] {
            assert!(tool.bracket_event(&ctx, &e, &mut ed, view, 1., hole).unwrap());
        }
        assert_eq!(ed.rev, rev + 1);
        assert!(
            matches!(ed.doc.typography.frames[&id].binding,Some(Binding::Path{start,..}) if (start-60.).abs()<0.01)
        );
        ed.undo();
        assert_eq!(ed.doc.typography.frames[&id].binding, Some(binding));
        assert!(tool.bracket_event(&ctx, &button(30., true), &mut ed, view, 1., hole).unwrap());
        let before = ed.doc.clone();
        tool.bracket_event(
            &ctx,
            &Event::Key {
                key: Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            },
            &mut ed,
            view,
            1.,
            hole,
        )
        .unwrap();
        assert!(tool.path_drag.is_none());
        assert_eq!(ed.doc, before);
    }
}
