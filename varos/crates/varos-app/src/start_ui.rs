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
                let width = ui.available_width().min(t::START_WIDTH);
                let inset = ((ui.available_width() - width) / 2.0).max(0.0);
                ui.horizontal_top(|ui| {
                    ui.add_space(inset);
                    ui.allocate_ui_with_layout(
                        egui::vec2(width, 0.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            ui.add_space(t::START_PAD);
                            if width >= t::START_WIDE {
                                ui.horizontal_top(|ui| {
                                    ui.allocate_ui_with_layout(
                                        egui::vec2(t::START_SIDEBAR, 0.0),
                                        egui::Layout::top_down(egui::Align::Min),
                                        |ui| {
                                            ui.set_max_width(t::START_SIDEBAR);
                                            self.launch(ui, &mut actions);
                                        },
                                    );
                                    ui.add_space(t::START_PAD);
                                    let body_width =
                                        (width - t::START_SIDEBAR - t::START_PAD - t::KIT_GAP).max(t::KIT_MIN_TARGET);
                                    ui.allocate_ui_with_layout(
                                        egui::vec2(body_width, 0.0),
                                        egui::Layout::top_down(egui::Align::Min),
                                        |ui| {
                                            ui.set_max_width(body_width);
                                            self.documents(ui, warning, focus_moved, &mut actions);
                                        },
                                    );
                                });
                            } else {
                                self.launch(ui, &mut actions);
                                ui.add_space(t::START_PAD);
                                self.documents(ui, warning, focus_moved, &mut actions);
                            }
                        },
                    );
                });
            });
        });
        actions
    }

    fn launch(&mut self, ui: &mut egui::Ui, actions: &mut Vec<StartAction>) {
        ui.label(egui::RichText::new(START_TITLE).size(t::START_TITLE_SIZE).color(t::TEXT));
        ui.add_space(t::START_GAP);
        for (index, label, icon, shortcut, action) in
            [(0, "New document", Icon::New, "N", StartAction::New), (1, "Open…", Icon::Open, "O", StartAction::Open)]
        {
            let hint = t::shortcut_label(shortcut);
            let mut c = Control::new(Id::new(("start-action", index)), label);
            c.icon = Some(icon);
            c.help = &hint;
            c.pointer_only = true;
            c.focused = self.keyboard_focus && self.model.focus() == index;
            ui.horizontal(|ui| {
                let r = kit::action(ui, c, false);
                if r.activated {
                    self.model.set_focus(index);
                    actions.push(action);
                }
                ui.label(egui::RichText::new(hint.as_str()).small().color(t::MUTED));
            });
            ui.add_space(t::KIT_GAP);
        }
    }

    fn documents(
        &mut self,
        ui: &mut egui::Ui,
        warning: Option<&str>,
        focus_moved: bool,
        actions: &mut Vec<StartAction>,
    ) {
        if let Some(warning) = warning {
            kit::notice(ui, warning);
            ui.add_space(t::START_GAP);
        }
        self.recovery(ui, actions);
        ui.label(egui::RichText::new("Recent documents").size(t::START_SECTION_SIZE).color(t::TEXT));
        ui.add_space(t::KIT_GAP);
        if self.model.rows().is_empty() {
            ui.separator();
            ui.add_space(t::START_EMPTY_PAD);
            ui.label(egui::RichText::new("Your next document starts here.").size(t::START_FILE_SIZE).color(t::TEXT));
            kit::notice(ui, EMPTY_RECENT_COPY);
            ui.add_space(t::START_EMPTY_PAD);
            return;
        }
        let wide = ui.available_width() >= t::START_DATE_BREAK;
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!(
                    "{} {}",
                    self.model.rows().len(),
                    if self.model.rows().len() == 1 { "document" } else { "documents" }
                ))
                .small()
                .color(t::MUTED),
            );
            if wide {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(t::KIT_CONTROL_H + t::KIT_PAD);
                    ui.label(egui::RichText::new("Last opened").small().color(t::MUTED));
                });
            }
        });
        ui.separator();
        let recent_start = 2 + if self.model.recovery().is_empty() { 0 } else { 2 * self.model.recovery().len() + 1 };
        let mut clicked = None;
        for (i, row) in self.model.rows().iter().enumerate() {
            let full_path = row.path.to_string_lossy();
            let mut c = Control::new(Id::new(("start-recent", &row.path)), &row.name);
            c.help = &full_path;
            c.icon = Some(Icon::Document);
            c.pointer_only = true;
            c.focused = self.keyboard_focus && self.model.focus() == i + recent_start;
            let detail = if row.missing {
                format!("Missing · {}", row.dir_elided)
            } else if wide {
                row.dir_elided.clone()
            } else {
                format!("{} · {}", row.when_text, row.dir_elided)
            };
            ui.horizontal(|ui| {
                let row_width = (ui.available_width() - t::KIT_CONTROL_H - t::KIT_GAP).max(t::KIT_MIN_TARGET);
                ui.allocate_ui_with_layout(
                    egui::vec2(row_width, t::START_ROW_H),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_max_width(row_width);
                        let r = kit::document_row(ui, c, &detail, if wide { &row.when_text } else { "" });
                        if focus_moved && self.keyboard_focus && self.model.focus() == i + recent_start {
                            r.response.scroll_to_me(None);
                        }
                        if r.activated {
                            clicked = Some(i + recent_start);
                            actions.push(StartAction::OpenRecent(row.path.clone()));
                        }
                        r.response.context_menu(|ui| recent_menu(ui, row, actions));
                    },
                );
                let mut more = Control::new(Id::new(("start-more", &row.path)), "Document actions");
                more.icon = Some(Icon::More);
                more.help = "Locate or remove from Recent";
                more.pointer_only = true;
                let r = kit::action(ui, more, true);
                egui::Popup::menu(&r.response).show(|ui| recent_menu(ui, row, actions));
            });
        }
        if let Some(index) = clicked {
            self.model.set_focus(index);
        }
        ui.add_space(t::START_GAP);
        ui.horizontal(|ui| {
            let mut c = Control::new(Id::new("start-clear"), "Clear Recent");
            c.pointer_only = true;
            c.focused = self.keyboard_focus && self.model.focus() == self.model.focus_count() - 1;
            if kit::action(ui, c, false).activated {
                actions.push(StartAction::ClearRecent);
            }
            kit::notice(ui, "Files stay on your computer.");
        });
    }

    fn recovery(&mut self, ui: &mut egui::Ui, actions: &mut Vec<StartAction>) {
        if self.model.recovery().is_empty() {
            return;
        }
        let mut clicked = None;
        kit::section_heading(ui, "Recovery copies");
        for (i, row) in self.model.recovery().iter().enumerate() {
            ui.label(&row.name);
            kit::notice(
                ui,
                &format!("{} · {}", row.original_dir.as_deref().unwrap_or("Not saved yet"), row.saved_at_text),
            );
            if let Some(reason) = &row.problem {
                kit::notice(ui, reason);
            }
            ui.horizontal(|ui| {
                for (offset, label, action) in [
                    (0, "Recover", StartAction::Recover(row.rid.clone())),
                    (1, "Discard…", StartAction::DiscardRecovery(row.rid.clone())),
                ] {
                    let index = 2 + i * 2 + offset;
                    let mut c = Control::new(Id::new(("start-recovery", &row.rid, offset)), label);
                    if row.busy {
                        c.availability = kit::Availability::Busy("Working…");
                    } else if offset == 0 {
                        if let Some(reason) = &row.problem {
                            c.availability = kit::Availability::Disabled(reason);
                        }
                    }
                    c.pointer_only = true;
                    c.focused = self.keyboard_focus && self.model.focus() == index;
                    if kit::action(ui, c, false).activated {
                        clicked = Some(index);
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
            clicked = Some(index);
            actions.push(StartAction::Later);
        }
        if let Some(index) = clicked {
            self.model.set_focus(index);
        }
        ui.add_space(t::START_GAP);
    }
}

fn recent_menu(ui: &mut egui::Ui, row: &crate::start::StartRow, actions: &mut Vec<StartAction>) {
    if row.missing && ui.button("Locate…").clicked() {
        actions.push(StartAction::Locate(row.path.clone()));
        ui.close();
    }
    if ui.button("Remove from Recent").clicked() {
        actions.push(StartAction::RemoveRecent(row.path.clone()));
        ui.close();
    }
}
