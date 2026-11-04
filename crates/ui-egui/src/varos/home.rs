//! Home's simple shell: local files, folder projects, templates and onboarding.
use crate::varos::theme_components::FOUNDATION as LOOK;
use crate::{
    VectorcraftApp, icons,
    theme::{self, Tokens},
};
use egui::{Align2, Color32, Rect, Response, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use serde_json::{Value, json};
use std::path::PathBuf;

#[path = "home_data.rs"]
mod data;
pub use data::Preferences;
pub(crate) use data::cache_document;
use data::{File, Sort};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Section {
    #[default]
    Recents,
    Projects,
    Templates,
    GettingStarted,
}
#[derive(Clone, Debug, Default)]
pub struct View {
    pub section: Section,
    pub query: String,
    pub project: Option<PathBuf>,
    pub selected: Option<String>,
    pub generation: u64,
}

/// Home yields as soon as New/Open/Close changes the document identity or count.
pub(crate) fn is_home(app: &VectorcraftApp) -> bool {
    app.ui.home.is_some_and(|key| key == crate::menus::home_key(app)) || app.session.active().is_none()
}

pub(crate) fn board_bounds(doc: &vectorcraft_doc::Document) -> Option<vectorcraft_geom::Rect> {
    doc.artboards.is_empty().then(|| vectorcraft_render::encode::art_bounds(doc).unwrap_or(vectorcraft_geom::Rect::new(-400.0, -300.0, 400.0, 300.0)))
}

/// Home operations share the existing command entry, so they are reachable from automation.
pub(crate) fn command(app: &mut VectorcraftApp, p: &Value) -> Result<Value, String> {
    let action = p["action"].as_str().unwrap_or("show");
    let path = || p["path"].as_str().ok_or_else(|| tl!("Home action needs a path").to_string());
    match action {
        "back" => {
            // Home overlays the existing editor; returning must not activate, reopen or
            // otherwise mutate a document. With no document, Home remains the only view.
            if app.session.active().is_some() {
                app.ui.home = None;
            }
            return Ok(Value::Null);
        }
        "open" => return crate::io::open_recent_reporting(app, path()?),
        "reveal" => return app.services.reveal.as_mut().ok_or(tl!("File manager unavailable"))?(path()?).map(|_| Value::Null),
        "removeRecent" => {
            let path = path()?;
            app.ui.recent_files.retain(|s| s != path);
        }
        "addProject" => {
            let chosen = p["path"].as_str().map(str::to_owned).or_else(|| crate::picks::folder(app));
            if let Some(path) = chosen {
                let path = PathBuf::from(path);
                vectorcraft_engine::file_access::check_read(&path.to_string_lossy())?;
                if !path.is_dir() {
                    return Err(tl!("Project folder not found").into());
                }
                let canonical = path.canonicalize().map_err(|e| e.to_string())?;
                let path = if canonical.to_str().is_some() { canonical } else { path };
                if !app.ui.home_preferences.projects.contains(&path) {
                    app.ui.home_preferences.projects.push(path);
                }
            }
        }
        "project" => {
            app.ui.home_view.project = Some(PathBuf::from(path()?));
            app.ui.home_view.section = Section::Projects;
        }
        "hideTips" => app.ui.home_preferences.tips_hidden = true,
        "showTips" => {
            app.ui.home_preferences.tips_hidden = false;
            app.ui.home_view.section = Section::GettingStarted;
        }
        "sort" => {
            app.ui.home_preferences.sort = match p["sort"].as_str() {
                Some("name") => Sort::Name,
                Some("created") => Sort::Created,
                Some("modified") => Sort::Modified,
                _ => return Err(tl!("Unknown Home sort order").into()),
            }
        }
        "show" => {
            app.ui.home_view.section = match p["section"].as_str() {
                Some("projects") => Section::Projects,
                Some("templates") => Section::Templates,
                Some("gettingStarted") => {
                    app.ui.home_preferences.tips_hidden = false;
                    Section::GettingStarted
                }
                Some("recents") | None => Section::Recents,
                _ => return Err(tl!("Unknown Home section").into()),
            };
            app.ui.home_view.project = None;
        }
        _ => return Err(tl!("Unknown Home action").into()),
    }
    app.ui.home_view.selected = None;
    app.ui.home_view.generation = app.ui.home_view.generation.wrapping_add(1);
    app.canvas_rect = None;
    app.ui.home = Some(crate::menus::home_key(app));
    Ok(Value::Null)
}

/// The host shortcut path calls this before tool keys; Home never nudges hidden artwork.
pub(crate) fn handle_shortcuts(app: &mut VectorcraftApp, ctx: &egui::Context) -> bool {
    if !is_home(app) {
        return false;
    }
    for id in ["file.newDialog", "file.newWithoutDialog", "file.open", "edit.preferences", "help.commandPalette"] {
        if let Some(shortcut) = crate::menus::shortcut_of(id).and_then(crate::shortcuts::parse)
            && ctx.input(|i| {
                i.events.iter().any(|e| {
                    matches!(e, egui::Event::Key { key, pressed: true, modifiers, .. }
                if *key == shortcut.logical_key && modifiers.matches_exact(shortcut.modifiers))
                })
            })
            && ctx.input_mut(|i| i.consume_shortcut(&shortcut))
        {
            run(app, id, json!({}));
            break;
        }
    }
    true
}

fn run(app: &mut VectorcraftApp, command: &str, params: Value) {
    if let Err(error) = app.run(command, params) {
        app.status(error);
    }
}
fn home_action(app: &mut VectorcraftApp, params: Value) {
    run(app, "app.home", params);
}

pub(crate) fn show(app: &mut VectorcraftApp, ui: &mut Ui, rect: Rect) {
    // Refresh relative dates, folder changes and completed thumbnail jobs while idle.
    ui.ctx().request_repaint_after(std::time::Duration::from_secs(2));
    let t = Tokens::get(ui.ctx());
    ui.painter().rect_filled(rect, 0.0, t.panel_darker);
    let top = Rect::from_min_size(rect.min, vec2(rect.width(), 40.0));
    top_bar(app, ui, top);
    ui.painter().hline(rect.x_range(), top.bottom(), Stroke::new(1.0, t.border));
    let body = Rect::from_min_max(pos2(rect.left(), top.bottom() + 1.0), rect.max);
    let side = Rect::from_min_size(body.min, vec2(220.0, body.height()));
    sidebar(app, ui, side);
    ui.painter().vline(side.right(), body.y_range(), Stroke::new(1.0, t.border));
    let main = Rect::from_min_max(pos2(side.right() + 1.0, body.top()), body.max);
    let inner = Rect::from_min_max(main.min + vec2(48.0, 36.0), main.max - vec2(48.0, 0.0));
    let mut child = ui.new_child(egui::UiBuilder::new().id_salt("home-main").max_rect(inner).layout(egui::Layout::top_down(egui::Align::Min)));
    child.set_clip_rect(main.intersect(ui.clip_rect()));
    child.spacing_mut().item_spacing = vec2(0.0, 0.0);
    let mut cards = Vec::new();
    egui::ScrollArea::vertical().id_salt("varos-home-simple").auto_shrink([false, false]).show(&mut child, |ui| {
        let (name, hour) = identity(ui.ctx());
        label_row(ui, &data::greeting(hour, &name), 28.0, theme::semibold(LOOK.home_title_font), t.text);
        ui.add_space(6.0);
        label_row(ui, tl!("Pick up where you left off, or start something new."), 17.0, egui::FontId::proportional(LOOK.panel_body_font), t.text_dim);
        ui.add_space(22.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            if action(ui, tl!("New board"), Some("plus"), true, false).clicked() {
                run(app, "file.newBoard", json!({}));
            }
            if action(ui, tl!("Open…"), None, false, false).clicked() {
                run(app, "file.open", json!({}));
            }
            action(ui, tl!("Join with a code"), None, false, true);
        });
        ui.add_space(34.0);
        let searching = !app.ui.home_view.query.trim().is_empty();
        let section = app.ui.home_view.section.clone();
        if searching || section == Section::Recents {
            recent(app, ui, &mut cards);
        }
        if searching || matches!(section, Section::Recents | Section::Projects) {
            projects(app, ui, &mut cards);
        }
        if searching || section == Section::Templates {
            templates(app, ui, &mut cards);
        }
        if !searching && !app.ui.home_preferences.tips_hidden && matches!(section, Section::Recents | Section::GettingStarted) {
            tips(app, ui);
        }
        card_keys(app, ui, &cards);
        ui.add_space(36.0);
    });
    // Home hides the editor status bar; errors still need a visible destination.
    let toast_id = egui::Id::unique("home-toast");
    let now = ui.input(|i| i.time);
    let (message, since) =
        ui.ctx().data(|d| d.get_temp::<(String, f64)>(toast_id)).filter(|(s, _)| s == &app.ui.status).unwrap_or_else(|| (app.ui.status.clone(), now));
    ui.ctx().data_mut(|d| d.insert_temp(toast_id, (message.clone(), since)));
    if !message.is_empty() && now - since < 6.0 {
        ui.ctx().request_repaint_after(std::time::Duration::from_secs_f64((6.0 - (now - since)).max(0.0)));
        let toast = Rect::from_min_size(pos2(main.left() + 48.0, main.bottom() - 46.0), vec2((main.width() - 96.0).max(1.0), 32.0));
        ui.painter().rect_filled(toast, f32::from(LOOK.box_radius), t.panel);
        text(
            ui,
            toast.shrink2(vec2(10.0, 0.0)),
            &crate::i18n::msg(&app.ui.status),
            egui::FontId::proportional(LOOK.control_font),
            t.text_dim,
            Align2::LEFT_CENTER,
        );
    }
}

fn identity(ctx: &egui::Context) -> (String, u32) {
    let id = egui::Id::unique("home-identity");
    let minute = vectorcraft_doc::metadata::now_unix().unwrap_or_default() / 60;
    if let Some((at, name, hour)) = ctx.data(|d| d.get_temp::<(i64, String, u32)>(id))
        && at == minute
    {
        return (name, hour);
    }
    let value = (data::user_name(), data::local_hour());
    ctx.data_mut(|d| d.insert_temp(id, (minute, value.0.clone(), value.1)));
    value
}

fn top_bar(app: &mut VectorcraftApp, ui: &mut Ui, rect: Rect) {
    let t = Tokens::get(ui.ctx());
    // Home's bar is the window's only bar: it drags the window (before its widgets register, so
    // they keep their clicks), clears the macOS window buttons and holds the caption buttons.
    if app.custom_titlebar || cfg!(target_os = "macos") {
        crate::titlebar::drag_area(ui, rect);
    }
    let lead = app.titlebar_inset.max(10.0);
    let trail = if app.custom_titlebar { crate::titlebar::WIDTH } else { 0.0 };
    let brand = Rect::from_min_size(rect.min + vec2(lead, 9.0), vec2(22.0, 22.0));
    crate::brand::paint_mark(ui, brand);
    text(
        ui,
        Rect::from_min_size(rect.min + vec2(lead + 30.0, 0.0), vec2(100.0, 40.0)),
        tl!("Varos"),
        theme::semibold(LOOK.panel_body_font),
        t.text,
        Align2::LEFT_CENTER,
    );
    let avatar = Rect::from_center_size(pos2(rect.right() - trail - 21.0, rect.center().y), vec2(22.0, 22.0));
    ui.painter().circle_filled(avatar.center(), 11.0, t.button);
    let (name, _) = identity(ui.ctx());
    let initial = name.chars().next().map(|c| c.to_uppercase().collect::<String>()).unwrap_or_default();
    text(ui, avatar, &initial, theme::semibold(LOOK.avatar_font), t.text, Align2::CENTER_CENTER);
    ui.interact(avatar, ui.scope_id().with_salt(egui::IdSalt::new("home-avatar")), Sense::hover()).on_hover_text(name);
    let search = Rect::from_min_size(pos2(avatar.left() - 276.0, rect.top() + 8.0), vec2(260.0, 24.0));
    ui.painter().rect_filled(search, f32::from(LOOK.control_radius), t.input);
    icons::paint(ui, "home-search", Rect::from_center_size(pos2(search.left() + 13.0, search.center().y), vec2(13.0, 13.0)), t.text_disabled);
    let old = app.ui.home_view.query.clone();
    let search_response = ui.put(
        search.shrink2(vec2(8.0, 2.0)).with_min_x(search.left() + 26.0),
        egui::TextEdit::singleline(&mut app.ui.home_view.query)
            .id(egui::Id::unique("home-search"))
            .hint_text(tl!("Search files and templates"))
            .font(egui::FontId::proportional(LOOK.control_font))
            .frame(egui::Frame::NONE),
    );
    if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
        app.ui.home_view.query.clear();
        search_response.surrender_focus();
    }
    if app.ui.home_view.query != old {
        app.ui.home_view.selected = None;
    }
    if app.custom_titlebar {
        crate::titlebar::caption_buttons(app, ui, rect);
    }
}

fn sidebar(app: &mut VectorcraftApp, ui: &mut Ui, rect: Rect) {
    let t = Tokens::get(ui.ctx());
    ui.painter().rect_filled(rect, 0.0, t.panel);
    let mut y = rect.top() + 12.0;
    if app.session.active().is_some() {
        let row = Rect::from_min_size(pos2(rect.left() + 8.0, y), vec2(204.0, 30.0));
        if nav_row(ui, row, tl!("Back to document"), "chevron-left", false, false).clicked() {
            home_action(app, json!({"action": "back"}));
        }
        y += 38.0;
        ui.painter().hline(rect.left() + 12.0..=rect.right() - 12.0, y, Stroke::new(1.0, t.border));
        y += 10.0;
    }
    for (label, icon, section, id) in [
        (tl!("Recents"), "home-clock", Some(Section::Recents), "recents"),
        (tl!("Projects"), "home-folder", Some(Section::Projects), "projects"),
        (tl!("Shared with me"), "home-users", None, "shared"),
        (tl!("Templates"), "home-template", Some(Section::Templates), "templates"),
        (tl!("Getting started"), "home-help", Some(Section::GettingStarted), "gettingStarted"),
    ] {
        if id == "gettingStarted" {
            ui.painter().hline(rect.left() + 12.0..=rect.right() - 12.0, y + 8.0, Stroke::new(1.0, t.border));
            y += 19.0;
        }
        let row = Rect::from_min_size(pos2(rect.left() + 8.0, y), vec2(204.0, 30.0));
        let r = nav_row(ui, row, label, icon, section.as_ref() == Some(&app.ui.home_view.section), section.is_none());
        if r.clicked() {
            home_action(app, json!({"section": id}));
        }
        y += 32.0;
    }
    let row = Rect::from_min_size(pos2(rect.left() + 8.0, rect.bottom() - 42.0), vec2(204.0, 30.0));
    if nav_row(ui, row, tl!("Preferences"), "home-settings", false, false).clicked() {
        run(app, "edit.preferences", json!({}));
    }
}

fn nav_row(ui: &mut Ui, rect: Rect, label: &str, icon: &str, active: bool, disabled: bool) -> Response {
    let t = Tokens::get(ui.ctx());
    let r = ui
        .add_enabled_ui(!disabled, |ui| {
            let r = ui.interact(rect, ui.scope_id().with_salt(egui::IdSalt::new(("home-nav", label))), Sense::click());
            r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), label));
            r
        })
        .inner;
    if active || r.hovered() {
        ui.painter().rect_filled(rect, f32::from(LOOK.control_radius), t.button);
    }
    let fg = if disabled { t.text_disabled } else { t.text_dim };
    icons::paint(ui, icon, Rect::from_center_size(pos2(rect.left() + 24.0, rect.center().y), vec2(16.0, 16.0)), fg);
    text(
        ui,
        Rect::from_min_max(rect.min + vec2(40.0, 0.0), rect.max - vec2(if disabled { 44.0 } else { 8.0 }, 0.0)),
        label,
        egui::FontId::proportional(LOOK.control_font),
        if disabled { fg } else { t.text },
        Align2::LEFT_CENTER,
    );
    if disabled {
        badge(ui, Rect::from_center_size(pos2(rect.right() - 28.0, rect.center().y), vec2(30.0, 16.0)));
    }
    r.on_disabled_hover_text(tl!("Sharing comes later"))
}
fn badge(ui: &Ui, rect: Rect) {
    let t = Tokens::get(ui.ctx());
    ui.painter().rect_stroke(rect, f32::from(LOOK.inset_radius), Stroke::new(1.0, t.button_border), StrokeKind::Inside);
    text(ui, rect, tl!("later"), egui::FontId::proportional(LOOK.badge_font), t.text_disabled, Align2::CENTER_CENTER);
}
fn action(ui: &mut Ui, label: &str, icon: Option<&str>, primary: bool, disabled: bool) -> Response {
    let t = Tokens::get(ui.ctx());
    let font = if primary { theme::semibold(LOOK.panel_body_font) } else { egui::FontId::proportional(LOOK.panel_body_font) };
    let width = ui.fonts_mut(|f| f.layout_no_wrap(label.to_owned(), font.clone(), t.text).size().x)
        + 28.0
        + if icon.is_some() { 16.0 } else { 0.0 }
        + if disabled { 38.0 } else { 0.0 };
    let r = ui
        .add_enabled_ui(!disabled, |ui| {
            let (rect, r) = ui.allocate_exact_size(vec2(width, 32.0), Sense::click());
            r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), label));
            ui.painter().rect_filled(
                rect,
                f32::from(LOOK.control_radius),
                if primary {
                    t.text
                } else if r.hovered() {
                    t.hover
                } else {
                    t.button
                },
            );
            let fg = if primary {
                t.panel_darker
            } else if disabled {
                t.text_dim
            } else {
                t.text
            };
            let x = rect.left() + 14.0;
            if let Some(icon) = icon {
                icons::paint(ui, icon, Rect::from_center_size(pos2(x + 5.0, rect.center().y), vec2(12.0, 12.0)), fg);
            }
            text(
                ui,
                Rect::from_min_max(
                    pos2(x + if icon.is_some() { 16.0 } else { 0.0 }, rect.top()),
                    rect.max - vec2(if disabled { 48.0 } else { 14.0 }, 0.0),
                ),
                label,
                font,
                fg,
                Align2::LEFT_CENTER,
            );
            if disabled {
                badge(ui, Rect::from_center_size(pos2(rect.right() - 29.0, rect.center().y), vec2(30.0, 16.0)));
            }
            focus(ui, rect, &r);
            r
        })
        .inner;
    r.on_disabled_hover_text(tl!("Sharing comes later"))
}

fn text(ui: &Ui, rect: Rect, label: &str, font: egui::FontId, color: Color32, align: Align2) {
    // egui does not join Arabic glyphs. Use the existing HarfRust-backed mask renderer.
    if label.chars().any(|c| ('\u{0600}'..='\u{06ff}').contains(&c)) {
        let db = super::theme::arabic_font_db();
        let em = db.face("IBM Plex Sans Arabic", "Regular").map_or(1.5, |face| {
            let (asc, desc) = face.vertical_metrics();
            ((asc + desc) / face.units_per_em()) as f32
        });
        let px = (font.size * em / 0.8 * ui.ctx().pixels_per_point()).round().clamp(12.0, 128.0) as u32;
        let id = egui::Id::unique(("home-shaped", label, px));
        let tex = ui.ctx().data(|d| d.get_temp::<egui::TextureHandle>(id)).or_else(|| {
            let image = super::arabic_fonts::render_in(db, "IBM Plex Sans Arabic", "Regular", label, px)?;
            // The sample renderer includes the font's tall em box. Crop transparent rows,
            // preserving the requested font size instead of squeezing that box into a row.
            let width = image.size[0];
            let top = image.pixels.iter().position(|p| p.a() > 0)? / width;
            let bottom = image.pixels.iter().rposition(|p| p.a() > 0)? / width + 1;
            let image = egui::ColorImage::new([width, bottom - top], image.pixels[top * width..bottom * width].to_vec());
            let tex = ui.ctx().load_texture("home-shaped", image, egui::TextureOptions::LINEAR);
            ui.ctx().data_mut(|d| d.insert_temp(id, tex.clone()));
            Some(tex)
        });
        if let Some(tex) = tex {
            let size = tex.size_vec2() / ui.ctx().pixels_per_point();
            let at = align.anchor_size(rect.min + vec2(rect.width() * align.x().to_factor(), rect.height() * align.y().to_factor()), size);
            ui.painter_at(rect).image(tex.id(), at, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), color);
            return;
        }
    }
    let mut job = egui::text::LayoutJob::simple_singleline(label.to_owned(), font, color);
    job.wrap.max_width = rect.width().max(0.0);
    job.wrap.max_rows = 1;
    job.wrap.overflow_character = Some('…');
    let galley = ui.fonts_mut(|f| f.layout_job(job));
    let at = align.anchor_size(rect.min + vec2(rect.width() * align.x().to_factor(), rect.height() * align.y().to_factor()), galley.size()).min;
    ui.painter_at(rect).galley(at, galley, color);
}
fn label_row(ui: &mut Ui, label: &str, height: f32, font: egui::FontId, color: Color32) {
    let (rect, r) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, ui.is_enabled(), label));
    text(ui, rect, label, font, color, Align2::LEFT_CENTER);
}
fn focus(ui: &Ui, rect: Rect, r: &Response) {
    if r.has_focus() {
        ui.painter().rect_stroke(rect, f32::from(LOOK.control_radius), Stroke::new(1.0, Tokens::get(ui.ctx()).accent), StrokeKind::Inside);
    }
}
fn text_button(ui: &mut Ui, label: &str, arrow: bool) -> Response {
    let t = Tokens::get(ui.ctx());
    let font = egui::FontId::proportional(LOOK.control_font);
    let width = ui.fonts_mut(|f| f.layout_no_wrap(label.into(), font.clone(), t.text_dim).size().x) + if arrow { 14.0 } else { 0.0 };
    let (rect, r) = ui.allocate_exact_size(vec2(width, 17.0), Sense::click());
    r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), label));
    text(
        ui,
        rect.with_max_x(rect.right() - if arrow { 14.0 } else { 0.0 }),
        label,
        font,
        if r.hovered() { t.text } else { t.text_dim },
        Align2::LEFT_CENTER,
    );
    if arrow {
        icons::paint(ui, "chevron-down", Rect::from_center_size(pos2(rect.right() - 5.0, rect.center().y), vec2(10.0, 10.0)), t.text_dim);
    }
    focus(ui, rect, &r);
    r
}

fn heading(ui: &mut Ui, label: &str, right: impl FnOnce(&mut Ui)) {
    let t = Tokens::get(ui.ctx());
    ui.allocate_ui_with_layout(vec2(ui.available_width(), 17.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
        ui.label(egui::RichText::new(label).font(theme::semibold(LOOK.panel_body_font)).color(t.text));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), right);
    });
    ui.add_space(12.0);
}

#[derive(Clone)]
struct Snapshot {
    key: (Vec<String>, Vec<PathBuf>, u64),
    at: f64,
    recent: Vec<File>,
    projects: Vec<(PathBuf, bool, Vec<File>)>,
}
fn snapshot(app: &VectorcraftApp, ctx: &egui::Context) -> Snapshot {
    let key = (crate::io::recent_files(app).to_vec(), app.ui.home_preferences.projects.clone(), app.ui.home_view.generation);
    let id = egui::Id::unique("home-files");
    let now = ctx.input(|i| i.time);
    if let Some(s) = ctx.data(|d| d.get_temp::<Snapshot>(id))
        && s.key == key
        && now - s.at < 2.0
    {
        return s;
    }
    let s = Snapshot {
        recent: key.0.iter().map(|p| File::read(PathBuf::from(p))).collect(),
        projects: key.1.iter().map(|p| (p.clone(), p.is_dir(), data::project_files(p))).collect(),
        key,
        at: now,
    };
    ctx.data_mut(|d| d.insert_temp(id, s.clone()));
    s
}

fn recent(app: &mut VectorcraftApp, ui: &mut Ui, cards: &mut Vec<CardTarget>) {
    heading(ui, tl!("Recent"), |ui| {
        let r = text_button(ui, app.ui.home_preferences.sort.label(), true);
        egui::Popup::menu(&r).show(|ui| {
            for (order, id) in [(Sort::Modified, "modified"), (Sort::Name, "name"), (Sort::Created, "created")] {
                if ui.selectable_label(app.ui.home_preferences.sort == order, order.label()).clicked() {
                    home_action(app, json!({"action":"sort", "sort": id}));
                    ui.close();
                }
            }
        });
    });
    let mut files = snapshot(app, ui.ctx()).recent;
    files.retain(|f| f.matches(app.ui.home_view.query.trim()));
    data::sort(&mut files, app.ui.home_preferences.sort);
    file_cards(app, ui, &files, true, cards);
    ui.add_space(34.0);
}
fn projects(app: &mut VectorcraftApp, ui: &mut Ui, cards: &mut Vec<CardTarget>) {
    heading(ui, tl!("Projects"), |ui| {
        if text_button(ui, tl!("Add project…"), false).clicked() {
            home_action(app, json!({"action":"addProject"}));
        }
    });
    let query = app.ui.home_view.query.trim().to_lowercase();
    let entries = snapshot(app, ui.ctx()).projects;
    let empty = entries.is_empty();
    for (path, exists, mut files) in entries {
        let name = path.file_name().unwrap_or(path.as_os_str()).to_string_lossy();
        let matching = path.to_string_lossy().to_lowercase().contains(&query);
        let file_count = files.len();
        files.retain(|f| matching || f.matches(&query));
        if !matching && files.is_empty() {
            continue;
        }
        let t = Tokens::get(ui.ctx());
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 36.0), Sense::hover());
        let r = ui
            .add_enabled_ui(exists, |ui| {
                let r = ui.interact(rect, ui.scope_id().with_salt(egui::IdSalt::new(("home-project", &path))), Sense::click());
                r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), name.as_ref()));
                r
            })
            .inner;
        if r.hovered() {
            ui.painter().rect_filled(rect, f32::from(LOOK.control_radius), t.button);
        }
        let fg = if exists { t.text } else { t.text_disabled };
        icons::paint(ui, "home-folder", Rect::from_center_size(pos2(rect.left() + 18.0, rect.center().y), vec2(16.0, 16.0)), fg);
        text(
            ui,
            Rect::from_min_max(rect.min + vec2(34.0, 0.0), pos2(rect.center().x, rect.bottom())),
            &name,
            egui::FontId::proportional(LOOK.control_font),
            fg,
            Align2::LEFT_CENTER,
        );
        let count = crate::i18n::fmt(tl!("{n} files · {path}"), &[("n", &file_count.to_string()), ("path", &display_path(&path))]);
        let detail = if exists { count } else { tl!("Not found").into() };
        text(
            ui,
            Rect::from_min_max(pos2(rect.center().x, rect.top()), rect.max - vec2(10.0, 0.0)),
            &detail,
            egui::FontId::proportional(LOOK.control_font),
            t.text_disabled,
            Align2::RIGHT_CENTER,
        );
        focus(ui, rect, &r);
        if r.on_hover_text(path.to_string_lossy()).clicked() {
            home_action(app, json!({"action":"project", "path":path.to_str()}));
        }
        if exists && (app.ui.home_view.project.as_ref() == Some(&path) || !query.is_empty()) {
            data::sort(&mut files, app.ui.home_preferences.sort);
            ui.add_space(12.0);
            file_cards(app, ui, &files, false, cards);
            ui.add_space(12.0);
        }
    }
    if empty {
        label_row(
            ui,
            tl!("Add a folder to keep its boards together."),
            36.0,
            egui::FontId::proportional(LOOK.control_font),
            Tokens::get(ui.ctx()).text_dim,
        );
    }
    ui.add_space(34.0);
}
fn display_path(path: &std::path::Path) -> String {
    if let Some(home) = std::env::var_os("HOME")
        && let Ok(relative) = path.strip_prefix(home)
    {
        return format!("~/{}", relative.display());
    }
    path.to_string_lossy().into_owned()
}

#[derive(Clone)]
struct CardTarget {
    key: String,
    rect: Rect,
    command: &'static str,
    params: Value,
}
fn file_cards(app: &mut VectorcraftApp, ui: &mut Ui, files: &[File], recent: bool, cards: &mut Vec<CardTarget>) {
    if files.is_empty() {
        label_row(ui, tl!("No files to show."), 40.0, egui::FontId::proportional(LOOK.panel_body_font), Tokens::get(ui.ctx()).text_dim);
        return;
    }
    let (cols, width) = grid(ui.available_width());
    for row in files.chunks(cols) {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 14.0;
            for file in row {
                let detail = crate::i18n::fmt(
                    tl!("{edited} · {n} artboards"),
                    &[
                        ("edited", &data::edited(file.modified, std::time::SystemTime::now())),
                        ("n", &file.artboards.map_or_else(|| "—".into(), |n| n.to_string())),
                    ],
                );
                let key = format!("{}:{}", if recent { "recent" } else { "project" }, file.path.display());
                let (rect, r) = card(app, ui, &key, &file.name, &detail, width, file.thumbnail.as_deref(), None);
                cards.push(CardTarget { key, rect, command: "app.home", params: json!({"action":"open", "path":file.path.to_str()}) });
                r.context_menu(|ui| {
                    if ui.button(tl!("Open")).clicked() {
                        home_action(app, json!({"action":"open","path":file.path.to_str()}));
                        ui.close();
                    }
                    if ui.button(tl!("Show in Finder")).clicked() {
                        home_action(app, json!({"action":"reveal","path":file.path.to_str()}));
                        ui.close();
                    }
                    if recent && ui.button(tl!("Remove from Recents")).clicked() {
                        home_action(app, json!({"action":"removeRecent","path":file.path.to_str()}));
                        ui.close();
                    }
                });
                if r.on_hover_text(file.path.to_string_lossy()).clicked() {
                    home_action(app, json!({"action":"open","path":file.path.to_str()}));
                }
            }
        });
        ui.add_space(14.0);
    }
    // CSS grid's gap only separates rows; the section supplies the outer margin.
    ui.add_space(-14.0);
}
fn grid(width: f32) -> (usize, f32) {
    let cols = ((width + 14.0) / 194.0).floor().max(1.0) as usize;
    (cols, ((width - 14.0 * cols.saturating_sub(1) as f32) / cols as f32).max(180.0))
}

#[allow(clippy::too_many_arguments)]
fn card(
    app: &mut VectorcraftApp,
    ui: &mut Ui,
    key: &str,
    name: &str,
    detail: &str,
    width: f32,
    image: Option<&std::path::Path>,
    aspect: Option<f32>,
) -> (Rect, Response) {
    let t = Tokens::get(ui.ctx());
    let height = width * 0.75;
    let (rect, r) = ui.allocate_exact_size(vec2(width, height + 40.0), Sense::click());
    r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), name));
    let thumb = Rect::from_min_size(rect.min, vec2(width, height));
    ui.painter().rect_filled(thumb, f32::from(LOOK.card_radius), if r.hovered() { t.hover } else { t.button });
    ui.painter().rect_stroke(thumb, f32::from(LOOK.card_radius), Stroke::new(1.0, t.border), StrokeKind::Inside);
    let texture = image.and_then(|path| thumbnail(ui.ctx(), path));
    if let Some(texture) = texture {
        let size = texture.size_vec2();
        let fitted = size * ((width - 24.0) / size.x).min((height - 24.0) / size.y);
        egui::Image::new(&texture).corner_radius(LOOK.inset_radius).paint_at(ui, Rect::from_center_size(thumb.center(), fitted));
    } else {
        icons::paint(
            ui,
            if aspect.is_some() { "home-template" } else { "file" },
            Rect::from_center_size(thumb.center(), vec2(28.0, 28.0)),
            t.text_disabled,
        );
        if let Some(aspect) = aspect {
            let size = vec2(64.0 * aspect.min(1.0), 64.0 / aspect.max(1.0));
            ui.painter().rect_filled(Rect::from_center_size(thumb.center(), size), 2.0, t.text_dim);
        }
    }
    text(
        ui,
        Rect::from_min_max(pos2(rect.left(), thumb.bottom() + 8.0), pos2(rect.right(), thumb.bottom() + 25.0)),
        name,
        egui::FontId::proportional(LOOK.panel_body_font),
        t.text,
        Align2::LEFT_CENTER,
    );
    text(
        ui,
        Rect::from_min_max(pos2(rect.left(), thumb.bottom() + 25.0), rect.max),
        detail,
        egui::FontId::proportional(LOOK.control_font),
        t.text_disabled,
        Align2::LEFT_CENTER,
    );
    if r.has_focus() || app.ui.home_view.selected.as_deref() == Some(key) {
        ui.painter().rect_stroke(thumb, f32::from(LOOK.card_radius), Stroke::new(1.0, t.accent), StrokeKind::Inside);
    }
    if r.clicked() {
        app.ui.home_view.selected = Some(key.into());
    }
    (rect, r)
}
fn thumbnail(ctx: &egui::Context, path: &std::path::Path) -> Option<egui::TextureHandle> {
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    let id = egui::Id::unique(("home-thumb", path));
    if let Some((at, tex)) = ctx.data(|d| d.get_temp::<(std::time::SystemTime, egui::TextureHandle)>(id))
        && at == modified
    {
        return Some(tex);
    }
    let image = image::ImageReader::open(path).ok()?.decode().ok()?.to_rgba8();
    let pixels = egui::ColorImage::from_rgba_unmultiplied([image.width() as usize, image.height() as usize], image.as_raw());
    let tex = ctx.load_texture(path.to_string_lossy(), pixels, egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, (modified, tex.clone())));
    Some(tex)
}

fn templates(app: &mut VectorcraftApp, ui: &mut Ui, cards: &mut Vec<CardTarget>) {
    heading(ui, tl!("Templates"), |_| {});
    let choices = [
        (tl!("A4 poster"), tl!("210 × 297 mm"), json!({"preset":"A4","units":"mm"}), 210.0_f32 / 297.0),
        (tl!("Square post"), tl!("1080 × 1080 px"), json!({"preset":"Social Square Post 1080×1080"}), 1.0),
        (tl!("Business card"), tl!("85 × 55 mm"), json!({"width":"85 mm","height":"55 mm","units":"mm"}), 85.0 / 55.0),
    ];
    let query = app.ui.home_view.query.trim().to_lowercase();
    let choices: Vec<_> = choices.into_iter().filter(|(name, ..)| name.to_lowercase().contains(&query)).collect();
    let (cols, width) = grid(ui.available_width());
    for row in choices.chunks(cols) {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 14.0;
            for (name, size, params, aspect) in row {
                let key = format!("template:{name}");
                let (rect, r) = card(app, ui, &key, name, size, width, None, Some(*aspect));
                cards.push(CardTarget { key, rect, command: "file.new", params: params.clone() });
                if r.clicked() {
                    run(app, "file.new", params.clone());
                }
            }
        });
        ui.add_space(14.0);
    }
    ui.add_space(34.0);
}
fn tips(app: &mut VectorcraftApp, ui: &mut Ui) {
    heading(ui, tl!("Getting started"), |ui| {
        if text_button(ui, tl!("Hide"), false).clicked() {
            home_action(app, json!({"action":"hideTips"}));
        }
    });
    let t = Tokens::get(ui.ctx());
    let width = ((ui.available_width() - 28.0) / 3.0).max(1.0);
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 14.0;
        for (title, detail) in [
            (tl!("Draw your first artboard"), tl!("Pick the Artboard tool, drag on the canvas or choose a preset size.")),
            (tl!("Write in Arabic"), tl!("The Type tool shapes Arabic natively; Kashida lives in the Character panel.")),
            (tl!("Let an agent help"), tl!("Connect Claude or Codex through Bridge; their work shows in orange on the canvas.")),
        ] {
            egui::Frame::NONE.fill(t.panel).stroke(Stroke::new(1.0, t.border)).corner_radius(LOOK.box_radius).inner_margin(14.0).show(ui, |ui| {
                ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                    ui.set_width((width - 30.0).max(1.0));
                    ui.set_min_height(56.0);
                    ui.label(egui::RichText::new(title).font(theme::semibold(LOOK.panel_body_font)).color(t.text));
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(detail).size(LOOK.control_font).color(t.text_dim));
                });
            });
        }
    });
}
fn card_keys(app: &mut VectorcraftApp, ui: &Ui, cards: &[CardTarget]) {
    if cards.is_empty() || ui.ctx().memory(|m| m.focused()) == Some(egui::Id::unique("home-search")) {
        return;
    }
    let key = ui.input_mut(|i| {
        [egui::Key::ArrowRight, egui::Key::ArrowLeft, egui::Key::ArrowDown, egui::Key::ArrowUp, egui::Key::Enter]
            .into_iter()
            .find(|k| i.consume_key(egui::Modifiers::NONE, *k))
    });
    let Some(key) = key else { return };
    let selected = cards.iter().position(|c| Some(c.key.as_str()) == app.ui.home_view.selected.as_deref());
    let index = selected.unwrap_or(0);
    if key == egui::Key::Enter {
        if let Some(card) = selected.and_then(|i| cards.get(i)) {
            run(app, card.command, card.params.clone());
        }
        return;
    }
    let target = if selected.is_none() {
        0
    } else if matches!(key, egui::Key::ArrowLeft | egui::Key::ArrowRight) {
        if key == egui::Key::ArrowLeft { index.saturating_sub(1) } else { (index + 1).min(cards.len() - 1) }
    } else {
        let current = &cards[index];
        cards
            .iter()
            .enumerate()
            .filter(
                |(_, c)| if key == egui::Key::ArrowDown { c.rect.top() > current.rect.top() + 1.0 } else { c.rect.top() < current.rect.top() - 1.0 },
            )
            .min_by(|(_, a), (_, b)| {
                let score =
                    |c: &CardTarget| (c.rect.top() - current.rect.top()).abs() * 10000.0 + (c.rect.center().x - current.rect.center().x).abs();
                score(a).total_cmp(&score(b))
            })
            .map_or(index, |(i, _)| i)
    };
    if let Some(card) = cards.get(target) {
        app.ui.home_view.selected = Some(card.key.clone());
        ui.scroll_to_rect(card.rect, None);
    }
}

#[cfg(test)]
#[path = "home_capture.rs"]
pub(crate) mod capture;
#[cfg(test)]
#[path = "tests_home.rs"]
pub(crate) mod tests;
