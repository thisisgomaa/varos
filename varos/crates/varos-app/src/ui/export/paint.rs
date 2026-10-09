//! Hand-painted Minimal sheet; card pixels are cached after one CPU worker pass per snapshot.
use super::{ExportSheet, Phase, SheetAction};
use egui::{Id, RichText};
use std::sync::{Arc, Mutex};
use varos_app::shell::{
    kit::{self, Availability, Control, Icon, MenuEntry},
    tokens as t,
};
use varos_raster::export::Format;

pub fn minimal(ui: &mut egui::Ui, sheet: &mut ExportSheet, action: &mut SheetAction) {
    let running = matches!(sheet.phase, Phase::Running { .. });
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(t::EXPORT_LEFT_W - t::KIT_PAD * 2.0);
            ui.horizontal(|ui| {
                for (selection, label) in [(false, "Artboards"), (true, "Selection")] {
                    let mut c = Control::new(Id::new(("export-tab", selection)), label);
                    c.selected = sheet.minimal.selection_tab == selection;
                    if running {
                        c.availability = Availability::Disabled("Exporting…");
                    } else if selection {
                        if let Some(reason) = sheet.minimal.selection_reason() {
                            c.availability = Availability::Disabled(reason);
                        }
                    }
                    if kit::action(ui, c, false).activated {
                        sheet.minimal.selection_tab = selection;
                    }
                }
            });
            ui.horizontal(|ui| {
                for (list, icon, label) in [(false, Icon::ExportGrid, "Grid"), (true, Icon::List, "List")] {
                    let mut c = Control::new(Id::new(("export-layout", list)), label);
                    c.icon = Some(icon);
                    c.selected = list == sheet.minimal.list;
                    if running {
                        c.availability = Availability::Disabled("Exporting…");
                    }
                    if kit::action(ui, c, true).activated {
                        sheet.minimal.list = list;
                    }
                }
            });
            egui::ScrollArea::vertical().id_salt("export-cards-scroll").max_height(t::EXPORT_GRID_H).show(ui, |ui| {
                let count = sheet.minimal.cards().assets.len();
                if count == 0 {
                    kit::notice(ui, "There is no visible artwork to export.");
                }
                if sheet.minimal.list {
                    for index in 0..count {
                        card(ui, sheet, index, running);
                    }
                } else {
                    for start in (0..count).step_by(3) {
                        ui.horizontal(|ui| {
                            for index in start..(start + 3).min(count) {
                                card(ui, sheet, index, running);
                            }
                        });
                    }
                }
            });
            ui.horizontal(|ui| {
                let count = sheet.minimal.cards().count();
                ui.label(
                    RichText::new(format!("Selected {count} · {count} files"))
                        .font(t::numeric_value(t::T_MICRO))
                        .color(t::MUTED),
                );
                let mut c = Control::new(Id::new("export-clear"), "Clear");
                if running {
                    c.availability = Availability::Disabled("Exporting…");
                }
                if kit::action(ui, c, false).activated {
                    sheet.minimal.cards_mut().checked.fill(false);
                }
            });
        });
        ui.vertical(|ui| {
            ui.set_width(t::EXPORT_FIELD_W + t::KIT_CONTROL_H);
            ui.horizontal(|ui| {
                Icon::Group.paint(
                    ui.painter(),
                    ui.cursor().min + egui::vec2(t::ICON_SM, t::ICON_SM),
                    t::ICON_SM,
                    t::MUTED,
                );
                ui.add_space(t::ICON_SM * 2.0);
                kit::notice(ui, "EXPORT TO");
            });
            ui.horizontal(|ui| {
                let (_, rect) = ui.allocate_space(egui::vec2(t::EXPORT_FIELD_W, t::KIT_CONTROL_H));
                if running {
                    ui.painter().text(
                        rect.left_center(),
                        egui::Align2::LEFT_CENTER,
                        &sheet.minimal.folder,
                        t::numeric_value(t::T_MICRO),
                        t::MUTED,
                    );
                } else {
                    let edit = kit::field::text_field(
                        ui,
                        kit::field::TextField {
                            id: Id::new("export-folder"),
                            rect,
                            value: &sheet.minimal.folder,
                            font: t::numeric_value(t::T_MICRO),
                            framed: true,
                            open: false,
                            hint: "Folder",
                        },
                        |s| {
                            if std::path::Path::new(s).is_absolute() {
                                Ok(s.to_owned())
                            } else {
                                Err("Choose an absolute folder path.")
                            }
                        },
                    );
                    if let Some(value) = edit.commit {
                        sheet.minimal.folder = value;
                    }
                }
                let mut c = Control::new(Id::new("export-folder-picker"), "…");
                c.help = "Choose export folder";
                if running {
                    c.availability = Availability::Disabled("Exporting…");
                }
                if kit::action(ui, c, false).activated {
                    if let Some(path) = rfd::FileDialog::new().set_directory(&sheet.minimal.folder).pick_folder() {
                        sheet.minimal.folder = path.to_string_lossy().into_owned();
                    }
                }
            });
            ui.add_space(t::KIT_GAP);
            kit::notice(ui, "FORMATS");
            ui.horizontal(|ui| {
                kit::notice(ui, "Format");
                let labels: Vec<_> = Format::ALL.iter().map(|f| f.label()).collect();
                if let Some(index) =
                    dropdown(ui, "export-format", sheet.minimal.options.format.label(), &labels, running)
                {
                    sheet.minimal.options.format = Format::ALL[index];
                }
                if matches!(sheet.minimal.options.format, Format::Png | Format::Jpeg) {
                    let mut c = Control::new(Id::new("export-options"), "Options");
                    c.help = "Format options";
                    if running {
                        c.availability = Availability::Disabled("Exporting…");
                    }
                    let response = kit::action(ui, c, false);
                    if response.activated {
                        ui.ctx().data_mut(|d| d.insert_temp(Id::new("export-options-anchor"), response.response.rect));
                        kit::toggle_menu_below(ui.ctx(), Id::new("export-options-popover"), response.response.rect);
                    }
                }
            });
            ui.horizontal(|ui| {
                kit::notice(ui, "Scale");
                if sheet.minimal.options.format.vector() {
                    kit::notice(ui, "vector");
                } else {
                    let scale = sheet.minimal.options.scale;
                    let label = if scale == 1.0 {
                        "1×".into()
                    } else if scale == 2.0 {
                        "2×".into()
                    } else if scale == 3.0 {
                        "3×".into()
                    } else {
                        format!("{} ppi", scale * 72.0)
                    };
                    if let Some(index) = dropdown(ui, "export-scale", &label, &["1×", "2×", "3×", "ppi…"], running)
                    {
                        sheet.minimal.options.scale = [1.0, 2.0, 3.0, 300.0 / 72.0][index];
                    }
                }
            });
            if !sheet.minimal.options.format.vector() && ![1.0, 2.0, 3.0].contains(&sheet.minimal.options.scale) {
                let edit = kit::field::number_field(
                    ui,
                    kit::field::NumberField {
                        id: Id::new("export-ppi"),
                        width: t::EXPORT_FIELD_W,
                        label: kit::field::Label::Letter("ppi"),
                        tip: "Pixels per inch",
                        value: sheet.minimal.options.scale * 72.0,
                        decimals: 0,
                        speed: 1.0,
                        range: 1.0..=4608.0,
                        disabled: running,
                    },
                );
                if let Some(ppi) = edit.commit.or(edit.live) {
                    sheet.minimal.options.scale = ppi / 72.0;
                }
            }
            options_popover(ui.ctx(), sheet);
        });
    });
    ui.add_space(t::KIT_GAP);
    ui.horizontal(|ui| {
        let mut advanced = Control::new(Id::new("export-advanced"), "▸ Advanced");
        advanced.availability = Availability::Disabled("coming");
        kit::action(ui, advanced, false);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let label = format!("Export {} files", sheet.minimal.cards().count());
            let mut c = Control::new(Id::new("export-screens"), &label);
            if running {
                c.availability = Availability::Busy("Exporting…");
            } else if sheet.busy {
                c.availability = Availability::Disabled(super::BUSY);
            } else if sheet.minimal.cards().count() == 0 {
                c.availability = Availability::Disabled("Select a card to export.");
            } else if sheet.minimal.folder.is_empty() {
                c.availability = Availability::Disabled("Choose an export folder.");
            }
            if kit::action(ui, c, false).activated {
                let ticket = crate::file_jobs::next_ticket();
                let cancel = crate::file_jobs::CancelFlag::default();
                let jobs = sheet.minimal.jobs(sheet.sid, ticket, cancel.clone());
                sheet.minimal.remaining = jobs.len();
                sheet.minimal.destinations.clear();
                sheet.minimal.report = Default::default();
                sheet.phase = Phase::Running { ticket, cancel: Some(cancel), cancelling: false };
                *action = SheetAction::Screens(sheet.sid, jobs);
            }
            if kit::action(ui, Control::new(Id::new("export-cancel"), "Cancel"), false).activated && !sheet.cancel() {
                *action = SheetAction::Close;
            }
        });
    });
    if ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::A)) && !running && !kit::field::any_open(ui.ctx()) {
        sheet.minimal.cards_mut().checked.fill(true);
    }
    sheet.minimal.remember();
}
fn dropdown(ui: &mut egui::Ui, key: &str, label: &str, labels: &[&str], disabled: bool) -> Option<usize> {
    let id = Id::new(key);
    let mut c = Control::new(id, label);
    c.icon = Some(Icon::ChevronDown);
    if disabled {
        c.availability = Availability::Disabled("Exporting…");
    }
    let r = kit::action(ui, c, false);
    if r.activated {
        kit::toggle_menu_below(ui.ctx(), id, r.response.rect);
    }
    let entries: Vec<_> = labels.iter().map(|label| MenuEntry::Item(label)).collect();
    kit::menu(ui.ctx(), id, &entries)
}
fn options_popover(ctx: &egui::Context, sheet: &mut ExportSheet) {
    let owner = Id::new("export-options-popover");
    if !kit::is_menu_open(ctx, owner) {
        return;
    }
    // Reuse kit menu's lifetime/escape handling for the PNG boolean; JPEG is a K3 field.
    if sheet.minimal.options.format == Format::Png {
        let entries = [MenuEntry::Item(if sheet.minimal.options.transparent {
            "✓ Transparent background"
        } else {
            "Transparent background"
        })];
        if kit::menu(ctx, owner, &entries).is_some() {
            sheet.minimal.options.transparent = !sheet.minimal.options.transparent;
        }
    } else if sheet.minimal.options.format == Format::Jpeg {
        let anchor = ctx.data(|d| d.get_temp::<egui::Rect>(Id::new("export-options-anchor")));
        let area = egui::Area::new(owner)
            .order(egui::Order::Tooltip)
            .fixed_pos(anchor.map(|r| r.left_bottom()).unwrap_or(ctx.content_rect().center()))
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(t::PANEL)
                    .stroke(egui::Stroke::new(t::KIT_STROKE, t::LINE2))
                    .corner_radius(t::r_box())
                    .show(ui, |ui| {
                        let edit = kit::field::number_field(
                            ui,
                            kit::field::NumberField {
                                id: Id::new("export-quality"),
                                width: t::EXPORT_FIELD_W,
                                label: kit::field::Label::Letter("Quality"),
                                tip: "JPEG quality 0–100",
                                value: sheet.minimal.options.quality as f32,
                                decimals: 0,
                                speed: 1.0,
                                range: 0.0..=100.0,
                                disabled: false,
                            },
                        );
                        if let Some(value) = edit.commit.or(edit.live) {
                            sheet.minimal.options.quality = value.round() as u8;
                        }
                        if kit::action(ui, Control::new(Id::new("export-quality-done"), "Done"), false).activated {
                            kit::close_menu(ctx);
                        }
                    });
            });
        let outside = ctx.input(|i| i.events.iter().any(|e| matches!(e,egui::Event::PointerButton { pos, pressed: true,.. } if !area.response.rect.contains(*pos) && anchor.is_none_or(|r| !r.contains(*pos)))));
        let escape =
            !kit::field::any_open(ctx) && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        if outside || escape {
            kit::close_menu(ctx);
        }
    } else {
        kit::close_menu(ctx);
    }
}
fn card(ui: &mut egui::Ui, sheet: &mut ExportSheet, index: usize, running: bool) {
    let asset = &sheet.minimal.cards().assets[index];
    let checked = sheet.minimal.cards().checked[index];
    let list = sheet.minimal.list;
    let size = if list {
        egui::vec2(t::EXPORT_LEFT_W - t::KIT_PAD * 2.0, t::KIT_CONTROL_H)
    } else {
        egui::vec2(t::EXPORT_CARD_W, t::EXPORT_CARD_H)
    };
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let fill = if checked {
        t::TOGGLE_WELL
    } else if response.hovered() {
        t::HOVER
    } else {
        t::SURFACE
    };
    ui.painter().rect(
        rect,
        t::R,
        fill,
        egui::Stroke::new(t::KIT_STROKE, if checked { t::ACCENT } else { t::LINE2 }),
        egui::StrokeKind::Inside,
    );
    let checkbox = egui::Rect::from_min_size(
        egui::pos2(rect.left() + t::KIT_PAD, rect.bottom() - t::EXPORT_CHECK - t::KIT_PAD),
        egui::vec2(t::EXPORT_CHECK, t::EXPORT_CHECK),
    );
    ui.painter().rect_filled(checkbox, t::R, if checked { t::ACCENT } else { t::PANEL });
    if checked {
        Icon::ExportCheck.paint(ui.painter(), checkbox.center(), t::EXPORT_CHECK, t::TEXT);
    }
    if list {
        ui.painter().text(
            egui::pos2(checkbox.right() + t::KIT_PAD, rect.center().y),
            egui::Align2::LEFT_CENTER,
            format!("{} · {} × {}", asset.name, asset.page.rect[2], asset.page.rect[3]),
            t::numeric_value(t::T_MICRO),
            t::TEXT,
        );
    } else {
        let preview = egui::Rect::from_min_size(rect.min, egui::vec2(t::EXPORT_CARD_W, t::EXPORT_CARD_W));
        thumbnail(ui, asset, sheet.minimal.preview_id, index, sheet.minimal.selection_tab, preview.shrink(t::KIT_PAD));
        ui.painter().text(
            egui::pos2(checkbox.right() + t::KIT_TEXT_GAP, rect.bottom() - t::KIT_PAD),
            egui::Align2::LEFT_BOTTOM,
            &asset.name,
            t::micro(),
            t::TEXT,
        );
    }
    response.clone().on_hover_text(&asset.name);
    if !running && (response.clicked() || response.double_clicked()) {
        sheet.minimal.cards_mut().click(index, ui.input(|i| i.modifiers.shift), response.double_clicked());
    }
}
#[derive(Clone, Default)]
struct Thumb {
    pending: Arc<Mutex<Option<egui::ColorImage>>>,
    texture: Option<egui::TextureHandle>,
}
fn thumbnail(
    ui: &egui::Ui,
    asset: &varos_raster::export::Asset,
    snapshot: u64,
    index: usize,
    selection: bool,
    rect: egui::Rect,
) {
    let id = Id::new(("export-thumbnail", snapshot, index, selection));
    let stored = ui.ctx().data(|d| d.get_temp::<Thumb>(id));
    let mut thumb = if let Some(t) = stored {
        t
    } else {
        let thumb = Thumb::default();
        let pending = thumb.pending.clone();
        let asset = asset.clone();
        let ctx = ui.ctx().clone();
        let service_id = Id::new("export-thumbnail-service");
        let service = ui.ctx().data_mut(|d| {
            d.get_temp::<Arc<Mutex<Option<crate::thumbs::ThumbService>>>>(service_id).unwrap_or_else(|| {
                let service = Arc::new(Mutex::new(
                    varos_app::storage::paths::AppLayout::current()
                        .and_then(|layout| crate::thumbs::ThumbService::at(layout.thumbs().join("Export"))),
                ));
                d.insert_temp(service_id, service.clone());
                service
            })
        });
        let key = crate::thumbs::ThumbKey(format!("export-{snapshot}-{selection}-{index}"));
        let mtime = std::time::SystemTime::UNIX_EPOCH;
        let accepted = service
            .lock()
            .ok()
            .and_then(|s| s.as_ref().map(|s| s.request_export(key.clone(), asset, mtime)))
            .unwrap_or(false);
        if !accepted {
            if service.lock().ok().is_none_or(|s| s.is_none()) {
                return;
            }
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(20));
            return;
        }
        std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
            while std::time::Instant::now() < deadline {
                let path =
                    service.lock().ok().and_then(|s| s.as_ref().and_then(|s| s.lookup(&key, mtime))).map(|lookup| {
                        match lookup {
                            crate::thumbs::Lookup::Fresh(path) | crate::thumbs::Lookup::Stale(path) => path,
                        }
                    });
                if let Some(path) = path {
                    if let Ok(bytes) = std::fs::read(path) {
                        if let Ok(image) = image::load_from_memory(&bytes) {
                            let rgba = image.to_rgba8();
                            let color = egui::ColorImage::from_rgba_unmultiplied(
                                [rgba.width() as usize, rgba.height() as usize],
                                rgba.as_raw(),
                            );
                            if let Ok(mut result) = pending.lock() {
                                *result = Some(color);
                            }
                        }
                    }
                    ctx.request_repaint();
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        });
        thumb
    };
    let pixels = thumb.pending.lock().ok().and_then(|mut p| p.take());
    if let Some(pixels) = pixels {
        thumb.texture = Some(ui.ctx().load_texture(
            format!("export-{snapshot}-{selection}-{index}"),
            pixels,
            egui::TextureOptions::LINEAR,
        ));
    }
    if let Some(texture) = &thumb.texture {
        let size = texture.size_vec2();
        let scale = (rect.width() / size.x).min(rect.height() / size.y);
        ui.painter().image(
            texture.id(),
            egui::Rect::from_center_size(rect.center(), size * scale),
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
            t::TEXT,
        );
    }
    ui.ctx().data_mut(|d| d.insert_temp(id, thumb));
}
