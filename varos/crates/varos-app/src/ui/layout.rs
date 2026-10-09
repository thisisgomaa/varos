//! Shell state only; document tabs, layer-node expansion and Start page stay outside this snapshot.
use super::Ui;
use varos_app::storage::layout::Layout;

impl Ui {
    /// Is the modeless colour panel open? Canvas shortcuts remain available.
    pub fn picker_open(&self) -> bool {
        self.color_panel.is_some()
    }
    pub fn picker_owns_escape(&self) -> bool {
        picker_owns_escape(&self.ctx, self.color_panel.is_some())
    }
    /// Is the picker eyedropper armed? The host swallows canvas clicks after feeding egui, so
    /// accepting a canvas sample cannot also select/deselect the artwork.
    pub fn picking_screen(&self) -> bool {
        self.color_panel.as_ref().is_some_and(|m| m.eyedropping)
    }

    pub(super) fn prepare_picker(&mut self, ed: &mut varos_core::editor::Editor) {
        self.ctx.data_mut(|d| d.insert_temp(egui::Id::new("picker-open"), self.picker_layout.open));
        if self.picker_layout.open && self.color_panel.is_none() {
            super::open_picker(&mut self.color_panel, super::MTarget::Paint(ed.paint), ed);
        }
        if let Some(m) = &mut self.color_panel {
            super::follow_selection(m, ed);
        }
    }
    pub(super) fn commit_picker_fields(&mut self, ed: &mut varos_core::editor::Editor) -> bool {
        settle_picker_fields(&self.ctx, self.doc_active, &mut self.field_pending, &mut self.color_panel, ed)
    }

    pub fn toggle_picker(&mut self, ed: &mut varos_core::editor::Editor) {
        if self.picker_layout.open {
            if let Some(m) = &mut self.color_panel {
                let mut ops = vec![];
                m.finish(&mut ops);
                super::apply_ops(ed, ops);
            }
            self.color_panel = None;
            self.picker_layout.open = false;
        } else {
            super::open_picker(&mut self.color_panel, super::MTarget::Paint(ed.paint), ed);
            self.picker_layout.open = true;
        }
    }
    pub fn arm_picker(&mut self, ed: &mut varos_core::editor::Editor) {
        if !self.commit_picker_fields(ed) {
            return;
        }
        if let Some(m) = &mut self.color_panel {
            m.arm();
        }
    }

    pub fn shell_layout(&self) -> Layout {
        Layout {
            tree: self.shell.layout_value(),
            show_rail: self.show_rail,
            show_control_bar: self.show_dock,
            picker: self.picker_layout.clone(),
        }
    }

    pub fn restore_shell_layout(&mut self, layout: Layout) {
        self.shell = varos_app::shell::ShellState::from_layout_value(layout.tree)
            .unwrap_or_else(varos_app::shell::ShellState::standard);
        self.picker_layout = layout.picker;
        self.color_panel = None;
        self.show_rail = layout.show_rail;
        self.show_dock = layout.show_control_bar;
        self.panel_column = None; // derived from restored splits on the first workspace pass
        self.ctx.request_repaint();
    }
}

pub(super) fn picker_owns_escape(ctx: &egui::Context, open: bool) -> bool {
    open && (ctx.data(|d| d.get_temp::<bool>(egui::Id::new("picker-mini")).unwrap_or(false))
        || varos_app::shell::kit::menu_open(ctx)
        || varos_app::shell::kit::field::any_open(ctx)
        || ctx.input(|i| i.pointer.hover_pos()).is_some_and(|p| {
            ctx.data(|d| d.get_temp::<egui::Rect>(egui::Id::new("picker-panel-rect"))).is_some_and(|r| r.contains(p))
        }))
}

/// Like the canvas press boundary: valid field text lands before a picker control's press,
/// so the new gesture starts from that colour and cannot fold the field into its snapshot.
pub(super) fn prepare_picker_input(
    ctx: &egui::Context,
    doc: Option<crate::app_command::SessionId>,
    pending: &mut Option<super::fields::Pending>,
    panel: &mut Option<super::ColorPanel>,
    ed: &mut varos_core::editor::Editor,
    input: &egui::RawInput,
) {
    if let Some(m) = panel {
        let rect = ctx.data(|d| d.get_temp::<egui::Rect>(egui::Id::new("picker-panel-rect")));
        if m.eyedropping
            && rect.is_some_and(|r| {
                input
                    .events
                    .iter()
                    .any(|e| matches!(e, egui::Event::PointerButton {pos, pressed:true,..} if r.contains(*pos)))
            })
        {
            let mut ops = vec![];
            m.finish(&mut ops);
            super::apply_ops(ed, ops);
            super::follow_selection(m, ed);
            m.disarmed_press = true;
        }
    }
    if panel.is_none() || !varos_app::shell::kit::field::any_open(ctx) {
        return;
    }
    let rect = ctx.memory(|m| m.focused()).and_then(|id| ctx.read_response(id)).map(|r| {
        r.rect.expand2(egui::vec2(varos_app::shell::tokens::FIELD_INSET_X, varos_app::shell::tokens::FIELD_INSET_Y))
    });
    let pressed_outside = input.events.iter().any(|event| match event {
        egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, .. } => {
            rect.is_some_and(|r| !r.contains(*pos))
        }
        _ => false,
    });
    if pressed_outside {
        let _ = settle_picker_fields(ctx, doc, pending, panel, ed);
    }
}

/// Finish a colour preview before the K3 field opens its own atomic transaction.
pub(super) fn settle_picker_fields(
    ctx: &egui::Context,
    doc: Option<crate::app_command::SessionId>,
    pending: &mut Option<super::fields::Pending>,
    panel: &mut Option<super::ColorPanel>,
    ed: &mut varos_core::editor::Editor,
) -> bool {
    if !varos_app::shell::kit::field::blocked(ctx) {
        if let Some(m) = panel {
            let mut ops = vec![];
            m.finish(&mut ops);
            super::apply_ops(ed, ops);
        }
    }
    super::fields::settle(ctx, doc, pending, ed)
}

#[cfg(test)]
mod tests;
