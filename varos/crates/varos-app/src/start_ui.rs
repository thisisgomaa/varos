//! Start presentation only. The host caches/rebuilds the pure model outside paint.
use crate::{
    shell::{
        kit::{self, Control, Icon, MenuEntry},
        tokens as t,
    },
    start::{StartAction, StartModel, EMPTY_RECENT_COPY, START_TITLE},
};
use egui::{Event, Id, Key};

pub struct StartPage {
    pub model: StartModel,
    /// The focus ring is visible (keyboard modality); a pointer press hides it.
    keyboard_focus: bool,
    /// Focus has been placed by the user (a Tab or a click). Until then focus rests on New and the
    /// first Tab only reveals it there, so the first Tab lands on the first control.
    entered: bool,
    /// The Recent row whose kit menu (Locate… / Remove from Recent) is open.
    menu_for: Option<std::path::PathBuf>,
}
impl StartPage {
    pub fn new(model: StartModel) -> Self {
        Self { model, keyboard_focus: false, entered: false, menu_for: None }
    }
    /// A rebuilt model (Recent changed, a probe finished). Keyboard focus and its visibility
    /// survive: the same element by key, or the row now in a removed row's place.
    pub fn replace(&mut self, model: StartModel) {
        let old = std::mem::replace(&mut self.model, model);
        self.model.carry_focus_from(&old);
        if self.menu_for.as_ref().is_some_and(|p| !self.model.rows().iter().any(|r| &r.path == p)) {
            self.menu_for = None;
        }
    }
    /// Home was (re)entered: start fresh, focus resting on New with no ring until the keyboard is used.
    pub fn reset_focus(&mut self) {
        self.model.set_focus(0);
        self.keyboard_focus = false;
        self.entered = false;
        self.menu_for = None;
    }
    /// Is the focus ring currently shown (keyboard modality)?
    pub fn keyboard_focus(&self) -> bool {
        self.keyboard_focus
    }
    pub fn draw(&mut self, ui: &mut egui::Ui, warning: Option<&str>) -> Vec<StartAction> {
        let mut actions = vec![];
        let mut focus_moved = false;
        // The pure model owns traversal. Consume navigation so egui does not also move focus.
        // While a menu is open it owns the keyboard (arrows / Enter / Esc).
        let blocked = egui::Popup::is_any_open(ui.ctx()) || kit::menu_open(ui.ctx());
        let events = if blocked { vec![] } else { ui.input(|i| i.events.clone()) };
        for event in events {
            match event {
                Event::PointerButton { pressed: true, .. } => self.keyboard_focus = false,
                Event::Key { key, pressed: true, repeat, modifiers, .. }
                    if !modifiers.command && !modifiers.ctrl && !modifiers.alt =>
                {
                    // Any key reveals the ring at once — no hidden first press.
                    self.keyboard_focus = true;
                    match key {
                        Key::Tab => {
                            if modifiers.shift {
                                self.model.tab_prev();
                            } else if self.entered {
                                self.model.tab_next();
                            }
                            self.entered = true;
                        }
                        Key::ArrowUp => self.model.arrow_up(),
                        Key::ArrowDown => self.model.arrow_down(),
                        Key::Enter | Key::Space if !repeat => actions.extend(self.model.activate()),
                        // Mac "delete" arrives as Backspace; both remove (same as the editor).
                        Key::Delete | Key::Backspace if !repeat => actions.extend(self.model.delete_focused()),
                        _ => continue,
                    }
                    focus_moved = true;
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
                    self.entered = true;
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
            kit::separator(ui);
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
        kit::separator(ui);
        let recent_start = 2 + if self.model.recovery().is_empty() { 0 } else { 2 * self.model.recovery().len() + 1 };
        let mut clicked = None;
        let mut open_menu = None;
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
                        if r.response.secondary_clicked() {
                            let at = r.response.interact_pointer_pos().unwrap_or(r.response.rect.center());
                            kit::open_menu(ui.ctx(), menu_owner(&row.path), at, None);
                            open_menu = Some(row.path.clone());
                        }
                    },
                );
                let mut more = Control::new(menu_owner(&row.path), "Document actions");
                more.icon = Some(Icon::More);
                more.help = "Locate or remove from Recent";
                more.pointer_only = true;
                let r = kit::action(ui, more, true);
                if r.activated {
                    kit::toggle_menu_below(ui.ctx(), menu_owner(&row.path), r.response.rect);
                    open_menu = Some(row.path.clone());
                }
            });
        }
        if let Some(index) = clicked {
            self.model.set_focus(index);
            self.entered = true;
        }
        if open_menu.is_some() {
            self.menu_for = open_menu;
        }
        self.recent_menu(ui.ctx(), actions);
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
            self.entered = true;
        }
        ui.add_space(t::START_GAP);
    }

    /// The open Recent row's kit menu: Locate… (missing files only), a hairline, Remove from Recent.
    fn recent_menu(&mut self, ctx: &egui::Context, actions: &mut Vec<StartAction>) {
        let Some(path) = self.menu_for.clone() else {
            return;
        };
        let owner = menu_owner(&path);
        let row = self.model.rows().iter().find(|r| r.path == path);
        let (Some(row), true) = (row, kit::is_menu_open(ctx, owner)) else {
            if kit::is_menu_open(ctx, owner) {
                kit::close_menu(ctx);
            }
            self.menu_for = None;
            return;
        };
        let entries: &[MenuEntry<'_>] = if row.missing {
            &[MenuEntry::Item(LOCATE), MenuEntry::Separator, MenuEntry::Item(REMOVE)]
        } else {
            &[MenuEntry::Item(REMOVE)]
        };
        if let Some(index) = kit::menu(ctx, owner, entries) {
            actions.push(match entries[index] {
                MenuEntry::Item(LOCATE) => StartAction::Locate(path),
                _ => StartAction::RemoveRecent(path),
            });
            self.menu_for = None;
        }
    }
}

const LOCATE: &str = "Locate…";
const REMOVE: &str = "Remove from Recent";

/// The row's "Document actions" control id doubles as its menu owner.
fn menu_owner(path: &std::path::Path) -> Id {
    Id::new(("start-more", path))
}
