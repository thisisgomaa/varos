//! Start presentation only. The host caches/rebuilds the pure model outside paint.
use crate::{
    shell::{
        kit::{self, Control, Icon},
        tokens as t,
    },
    start::{StartAction, StartModel, EMPTY_RECENT_COPY, START_TITLE},
};
use egui::{Event, Id, Key};

pub struct StartPage {
    pub model: StartModel,
    keyboard_focus: bool,
}
impl StartPage {
    pub fn new(model: StartModel) -> Self {
        Self { model, keyboard_focus: false }
    }
    pub fn replace(&mut self, model: StartModel) {
        self.model = model;
        self.keyboard_focus = false;
    }
    pub fn draw(&mut self, ui: &mut egui::Ui, warning: Option<&str>) -> Vec<StartAction> {
        let mut actions = vec![];
        let mut focus_moved = false;
        // The pure model owns traversal. Consume navigation so egui does not also move focus.
        let events = if egui::Popup::is_any_open(ui.ctx()) { vec![] } else { ui.input(|i| i.events.clone()) };
        for event in events {
            match event {
                Event::PointerButton { pressed: true, .. } => self.keyboard_focus = false,
                Event::Key { key, pressed: true, repeat, modifiers, .. }
                    if !modifiers.command && !modifiers.ctrl && !modifiers.alt =>
                {
                    match key {
                        Key::Tab => {
                            if modifiers.shift {
                                self.model.tab_prev();
                            } else {
                                self.model.tab_next();
                            }
                        }
                        Key::ArrowUp => self.model.arrow_up(),
                        Key::ArrowDown => self.model.arrow_down(),
                        Key::Enter | Key::Space if !repeat => actions.extend(self.model.activate()),
                        Key::Delete if !repeat => actions.extend(self.model.delete_focused()),
                        _ => continue,
                    }
                    focus_moved = true;
                    self.keyboard_focus = true;
                    ui.input_mut(|i| {
                        i.consume_key(modifiers, key);
                    });
                }
                _ => {}
            }
        }
        egui::Frame::new().fill(t::PANEL).inner_margin(t::START_PAD).show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.set_max_width(t::START_WIDTH);
                kit::section_heading(ui, START_TITLE);
                ui.add_space(t::START_GAP);
                ui.horizontal_wrapped(|ui| {
                    for (index, label, icon, action) in
                        [(0, "New document", Icon::New, StartAction::New), (1, "Open…", Icon::Open, StartAction::Open)]
                    {
                        let mut c = Control::new(Id::new(("start-action", index)), label);
                        c.icon = Some(icon);
                        c.pointer_only = true;
                        c.focused = self.keyboard_focus && self.model.focus() == index;
                        let r = kit::action(ui, c, false);
                        if r.activated {
                            self.model.set_focus(index);
                            actions.push(action);
                        }
                    }
                });
                ui.add_space(t::START_GAP);
                if let Some(warning) = warning {
                    kit::notice(ui, warning);
                    ui.add_space(t::START_GAP);
                }
                let mut recovery_clicked = None;
                if !self.model.recovery().is_empty() {
                    kit::section_heading(ui, "Recovery copies");
                    for (i, row) in self.model.recovery().iter().enumerate() {
                        ui.label(&row.name);
                        kit::notice(
                            ui,
                            &format!(
                                "{} · {}",
                                row.original_dir.as_deref().unwrap_or("Not saved yet"),
                                row.saved_at_text
                            ),
                        );
                        ui.horizontal(|ui| {
                            for (offset, label, action) in [
                                (0, "Recover", StartAction::Recover(row.rid.clone())),
                                (1, "Discard…", StartAction::DiscardRecovery(row.rid.clone())),
                            ] {
                                let index = 2 + i * 2 + offset;
                                let mut c = Control::new(Id::new(("start-recovery", &row.rid, offset)), label);
                                c.pointer_only = true;
                                c.focused = self.keyboard_focus && self.model.focus() == index;
                                if kit::action(ui, c, false).activated {
                                    recovery_clicked = Some(index);
                                    actions.push(action);
                                }
                            }
                        });
                    }
                    let index = 2 + self.model.recovery().len() * 2;
                    let mut c = Control::new(Id::new("start-later"), "Later");
                    c.pointer_only = true;
                    c.focused = self.keyboard_focus && self.model.focus() == index;
                    if kit::action(ui, c, false).activated {
                        recovery_clicked = Some(index);
                        actions.push(StartAction::Later);
                    }
                    ui.add_space(t::START_GAP);
                }
                if let Some(index) = recovery_clicked {
                    self.model.set_focus(index);
                }
                let recent_start =
                    2 + if self.model.recovery().is_empty() { 0 } else { 2 * self.model.recovery().len() + 1 };
                kit::section_heading(ui, "Recent documents");
                if self.model.rows().is_empty() {
                    kit::notice(ui, EMPTY_RECENT_COPY);
                }
                let mut clicked = None;
                for (i, row) in self.model.rows().iter().enumerate() {
                    let full_path = row.path.to_string_lossy();
                    let mut c = Control::new(Id::new(("start-recent", &row.path)), &row.name);
                    c.help = &full_path;
                    c.pointer_only = true;
                    c.focused = self.keyboard_focus && self.model.focus() == i + recent_start;
                    let detail = format!(
                        "{}{} · {}",
                        if row.missing { "Missing · " } else { "" },
                        row.dir_elided,
                        row.when_text
                    );
                    let r = kit::list_row(ui, c, &detail);
                    if focus_moved && c_focus(self.keyboard_focus, self.model.focus(), i + recent_start) {
                        r.response.scroll_to_me(None);
                    }
                    if r.activated {
                        clicked = Some(i + recent_start);
                        actions.push(StartAction::OpenRecent(row.path.clone()));
                    }
                    r.response.context_menu(|ui| {
                        if row.missing && ui.button("Locate…").clicked() {
                            actions.push(StartAction::Locate(row.path.clone()));
                            ui.close();
                        }
                        if ui.button("Remove from Recent").clicked() {
                            actions.push(StartAction::RemoveRecent(row.path.clone()));
                            ui.close();
                        }
                    });
                }
                if let Some(index) = clicked {
                    self.model.set_focus(index);
                }
                if !self.model.rows().is_empty() {
                    ui.add_space(t::START_GAP);
                    let mut c = Control::new(Id::new("start-clear"), "Clear Recent");
                    c.pointer_only = true;
                    c.focused = self.keyboard_focus && self.model.focus() == self.model.focus_count() - 1;
                    let r = kit::action(ui, c, false);
                    if r.activated {
                        actions.push(StartAction::ClearRecent);
                    }
                }
            });
        });
        actions
    }
}
fn c_focus(keyboard: bool, current: usize, row: usize) -> bool {
    keyboard && current == row
}
