//! A regular panel: shared by the original dock and the Varos shell.
use crate::{VectorcraftApp, icons, panels, theme::Tokens};
use egui::{Align, Button, Layout, Sense, Ui, vec2};
use serde_json::json;

#[derive(Clone, Default)]
struct Editing {
    page: String,
    name: String,
    focus: bool,
}

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let rename_key = format!("pages-rename-{}", app.session.active().map_or(0, |p| p.uid));
    let mut editing: Editing = panels::pstate(ui.ctx(), &rename_key);
    ui.horizontal(|ui| {
        // The Varos shell's tab already names the panel: only the add button stays.
        if !crate::varos::shell::enabled(app) {
            ui.strong("Pages");
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if crate::widgets::icon_button(ui, "plus", "Add page", false, 24.0).clicked() {
                run(app, "page.add", json!({}));
            }
        });
    });
    let Some(project) = app.session.active() else {
        ui.label("No project");
        return;
    };
    let uid = project.uid;
    let rows: Vec<_> = project
        .pages()
        .iter()
        .enumerate()
        .map(|(i, p)| (i, p.id.clone(), p.name.clone(), p.doc.doc.artboards.len(), i == project.active_page()))
        .collect();
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for (index, id, name, count, active) in &rows {
            ui.push_id((uid, id), |ui| {
                if editing.page == *id {
                    let response =
                        ui.add_sized(vec2(ui.available_width(), 28.0), egui::TextEdit::singleline(&mut editing.name).hint_text("Page name"));
                    if editing.focus {
                        response.request_focus();
                        editing.focus = false;
                    }
                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        editing = Editing::default();
                    } else if response.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        run(app, "page.rename", json!({"page": id, "name": editing.name}));
                        editing = Editing::default();
                    }
                    return;
                }
                let response = ui
                    .scope(|ui| {
                        let icon = egui::Image::new(icons::source("file")).fit_to_exact_size(vec2(16.0, 16.0)).tint(if *active {
                            t.accent
                        } else {
                            t.text_dim
                        });
                        let button = Button::image_and_text(icon, name)
                            .right_text(count.to_string())
                            .selected(*active)
                            .frame_when_inactive(*active)
                            .sense(Sense::click_and_drag());
                        let button = if *active { button.fill(t.accent.gamma_multiply(0.22)) } else { button };
                        ui.add_sized(vec2(ui.available_width(), 28.0), button)
                    })
                    .inner;
                if response.clicked() {
                    run(app, "page.switch", json!({"page": id}));
                }
                if response.double_clicked() {
                    editing = Editing { page: id.clone(), name: name.clone(), focus: true };
                }
                if response.drag_started() {
                    response.dnd_set_drag_payload((uid, id.clone()));
                }
                if let Some(payload) = response.dnd_release_payload::<(u64, String)>()
                    && payload.0 == uid
                    && payload.1 != *id
                {
                    run(app, "page.move", json!({"page": payload.1, "index": index}));
                }
                response.context_menu(|ui| {
                    if ui.button("Rename").clicked() {
                        editing = Editing { page: id.clone(), name: name.clone(), focus: true };
                        ui.close();
                    }
                    if ui.button("Duplicate").clicked() {
                        run(app, "page.duplicate", json!({"page": id}));
                        ui.close();
                    }
                    if ui.add_enabled(rows.len() > 1, Button::new("Delete")).clicked() {
                        run(app, "page.delete", json!({"page": id}));
                        ui.close();
                    }
                    if ui.add_enabled(*index > 0, Button::new("Move up")).clicked() {
                        run(app, "page.move", json!({"page": id, "index": index.saturating_sub(1)}));
                        ui.close();
                    }
                    if ui.add_enabled(*index + 1 < rows.len(), Button::new("Move down")).clicked() {
                        run(app, "page.move", json!({"page": id, "index": index + 1}));
                        ui.close();
                    }
                });
            });
        }
    });
    panels::set_pstate(ui.ctx(), &rename_key, editing);
}

/// Status-bar hook for the shell owner: this lane does not edit chrome.rs.
pub fn status(app: &VectorcraftApp) -> Option<String> {
    let p = app.session.active()?;
    let artboard = app.view().map_or(0, |v| v.artboard + 1).min(p.doc.artboards.len());
    Some(format!("Page {} of {} · Artboard {} / {}", p.active_page() + 1, p.pages().len(), artboard, p.doc.artboards.len()))
}

fn run(app: &mut VectorcraftApp, command: &str, params: serde_json::Value) {
    if let Err(error) = app.run(command, params) {
        app.ui.status = error;
    }
}
