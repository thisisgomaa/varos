//! The modeless Preferences window: upstream's category list and fields
//! ([`crate::prefs_dialog::category_fields`]) in a movable window with Apply, so the editor stays
//! live while it is open.
use serde_json::json;
use vectorcraft_engine::Prefs;
use vectorcraft_engine::cmd::prefscmds::PREF_CATEGORIES;

use super::{CATEGORY, Edit, apply, confirm, set_field};
use crate::theme::{self, Tokens};
use crate::{VectorcraftApp, widgets};

pub(crate) fn show(app: &mut VectorcraftApp, ctx: &egui::Context) {
    // The dialog is edited in place (taken for the frame, put back below), never copied.
    let Some(mut dialog) = app.ui.dialog.take() else { return };
    let mut d = Edit { dialog: &mut dialog, draft: &mut app.ui.prefs_draft };
    let t = Tokens::get(ctx);
    let (mut ok, mut apply_now, mut cancel, mut reset) = (false, false, false, false);
    let mut open = true;
    let screen = ctx.content_rect();
    let body_height = (screen.height() - 160.0).clamp(80.0, 430.0);
    let cat = d.str(CATEGORY);
    let cat = PREF_CATEGORIES.iter().find(|c| **c == cat).copied().unwrap_or(PREF_CATEGORIES[0]);
    egui::Window::new(tl!("Preferences"))
        .id(egui::Id::unique("dialog-preferences"))
        .order(egui::Order::Foreground)
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .movable(true)
        .default_pos(screen.center() - egui::vec2(400.0, (body_height + 110.0) * 0.5))
        .constrain_to(screen)
        .frame(egui::Frame::window(&ctx.global_style()).fill(t.panel).inner_margin(egui::Margin::same(20)))
        .show(ctx, |ui| {
            ui.set_width(760.0);
            ui.horizontal_top(|ui| {
                // Category list.
                egui::Frame::NONE.fill(t.panel_darker).corner_radius(egui::CornerRadius::same(4)).inner_margin(egui::Margin::same(6)).show(
                    ui,
                    |ui| {
                        ui.set_width(196.0);
                        egui::ScrollArea::vertical().id_salt("prefs-categories").max_height(body_height).auto_shrink([false, false]).show(ui, |ui| {
                            // ScrollArea inherits the surrounding horizontal layout; categories
                            // need their own vertical column rather than one very wide row.
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 1.0;
                                for c in PREF_CATEGORIES {
                                    let sel = *c == cat;
                                    let text = egui::RichText::new(tl!(*c)).size(12.5).color(if sel { t.text_strong } else { t.text });
                                    let b = egui::Button::selectable(sel, text).min_size(egui::vec2(184.0, 24.0));
                                    if ui.add(b).clicked() {
                                        d.set(CATEGORY, json!(c));
                                    }
                                }
                            });
                        });
                    },
                );
                ui.add_space(14.0);
                // Fields.
                ui.vertical(|ui| {
                    ui.set_width(530.0);
                    ui.label(egui::RichText::new(tl!(cat)).font(theme::semibold(14.0)).color(t.text_strong));
                    ui.add_space(8.0);
                    egui::ScrollArea::vertical().id_salt(("prefs", cat)).max_height((body_height - 30.0).max(50.0)).auto_shrink([false, false]).show(
                        ui,
                        |ui| {
                            crate::prefs_dialog::category_fields(ui, &mut d, cat);
                        },
                    );
                });
            });
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                if widgets::secondary_button(ui, tl!("Reset Preferences"))
                    .on_hover_text(tl!("Restore every preference to its default (applied on Apply or OK)"))
                    .clicked()
                {
                    reset = true;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::primary_button(ui, tl!("OK")).clicked() {
                        ok = true;
                    }
                    ui.add_space(8.0);
                    if widgets::secondary_button(ui, tl!("Apply")).clicked() {
                        apply_now = true;
                    }
                    ui.add_space(8.0);
                    if widgets::secondary_button(ui, tl!("Cancel")).clicked() {
                        cancel = true;
                    }
                    ui.add_space(16.0);
                    let i = PREF_CATEGORIES.iter().position(|c| *c == cat).unwrap_or(0);
                    if ui.add_enabled(i + 1 < PREF_CATEGORIES.len(), egui::Button::new(tl!("Next"))).clicked() {
                        d.set(CATEGORY, json!(PREF_CATEGORIES[i + 1]));
                    }
                    if ui.add_enabled(i > 0, egui::Button::new(tl!("Previous"))).clicked() {
                        d.set(CATEGORY, json!(PREF_CATEGORIES[i - 1]));
                    }
                });
            });
        });
    if reset {
        // Reset is the user's edit of every preference, even one already at its default.
        for (k, v) in Prefs::default().to_json().as_object().cloned().unwrap_or_default() {
            set_field(d.dialog, d.draft, k, v);
        }
    }
    app.ui.dialog = Some(dialog);
    if cancel || !open {
        crate::dialogs::cancel(app);
    } else if ok {
        if let Err(e) = confirm(app) {
            app.status(e);
        }
    } else if apply_now && let Err(e) = apply(app) {
        app.status(e);
    }
}
