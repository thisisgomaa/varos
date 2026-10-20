//! One Home predicate for welcome content and editor chrome.
use crate::VectorcraftApp;

/// Home may cover open documents until their identity/count changes (New/Open/Close).
/// Ignore a stale Home key before chrome layout, so a new document gets chrome immediately.
pub(crate) fn is_home(app: &VectorcraftApp) -> bool {
    app.ui.home.is_some_and(|key| key == crate::menus::home_key(app)) || app.session.active().is_none()
}

use crate::{
    icons,
    theme::{self, Tokens},
};
use egui::{Align2, Color32, CornerRadius, Rect, Response, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use serde_json::json;

/// The viewport of an empty free canvas is centred at the document ruler origin.
/// This is only a view target, never a hidden artboard or a persisted page.
pub(crate) fn board_bounds(doc: &vectorcraft_doc::Document) -> Option<vectorcraft_geom::Rect> {
    doc.artboards.is_empty().then(|| vectorcraft_render::encode::art_bounds(doc).unwrap_or(vectorcraft_geom::Rect::new(-400.0, -300.0, 400.0, 300.0)))
}

fn run(app: &mut VectorcraftApp, command: &str, params: serde_json::Value) {
    if let Err(error) = app.run(command, params) {
        app.ui.status = error;
    }
}

/// Quiet, bounded Home surface. The editor chrome shares `is_home` above.
pub(crate) fn show(app: &mut VectorcraftApp, ui: &mut Ui, rect: Rect) {
    let t = Tokens::get(ui.ctx());
    ui.painter().rect_filled(rect, 0.0, t.panel_darker);
    let margin = ((rect.width() - 1000.0) * 0.5).max(24.0);
    let inner = rect.shrink2(vec2(margin, 0.0));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::top_down(egui::Align::Min)));
    egui::ScrollArea::vertical().id_salt("varos-home").show(&mut child, |ui| {
        ui.add_space(48.0);
        greeting(ui);
        ui.add_space(22.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
            for (label, icon, command) in [
                (tl!("New file"), "file-plus", "file.newDialog"),
                (tl!("Open"), "folder-open", "file.open"),
                (tl!("New board"), "square-dashed", "file.newBoard"),
            ] {
                if action(ui, label, icon, command == "file.newDialog").clicked() {
                    run(app, command, json!({}));
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new("⌘N · ⌘O").size(11.0).color(t.text_dim));
            });
        });
        ui.add_space(36.0);
        let width = ui.available_width();
        if width >= 720.0 {
            ui.horizontal_top(|ui| {
                let left = (width - 28.0) * 0.54;
                ui.allocate_ui_with_layout(vec2(left, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| presets(app, ui));
                ui.add_space(20.0);
                ui.allocate_ui_with_layout(vec2((width - left - 28.0).max(0.0), 0.0), egui::Layout::top_down(egui::Align::Min), |ui| recent(app, ui));
            });
        } else {
            presets(app, ui);
            ui.add_space(28.0);
            recent(app, ui);
        }
        ui.add_space(24.0);
    });
}

const ARABIC_GREETING: &str = "أهلاً بك في فاروس";

fn greeting(ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), 44.0), Sense::hover());
    crate::brand::paint_mark(ui, Rect::from_center_size(pos2(row.left() + 17.0, row.center().y), vec2(34.0, 34.0)));
    let english = Rect::from_min_max(row.min + vec2(46.0, 0.0), pos2(row.right() - 180.0, row.bottom()));
    text(ui, english, tl!("Welcome to VectorCraft"), theme::semibold(26.0), t.text, Align2::LEFT_CENTER);
    let slot = Rect::from_min_max(pos2(row.right() - 170.0, row.top()), row.max);
    // egui does not shape Arabic. Reuse Varos' HarfRust sample rasterizer with the
    // already attributed bundled Plex face, never disconnected or reversed glyphs.
    let px = (40.0 * ui.ctx().pixels_per_point()).round().clamp(16.0, 96.0) as u32;
    let id = egui::Id::new(("home-arabic-greeting", px));
    let texture = ui.ctx().data(|d| d.get_temp::<egui::TextureHandle>(id)).or_else(|| {
        let image = super::arabic_fonts::render_in(super::theme::arabic_font_db(), "IBM Plex Sans Arabic", "Regular", ARABIC_GREETING, px)?;
        let tex = ui.ctx().load_texture("home-arabic-greeting", image, egui::TextureOptions::LINEAR);
        ui.ctx().data_mut(|d| d.insert_temp(id, tex.clone()));
        Some(tex)
    });
    if let Some(tex) = texture {
        super::arabic_fonts::paint_rtl(ui, slot, &tex, t.text_dim);
    }
}

fn text(ui: &Ui, rect: Rect, label: &str, font: egui::FontId, color: Color32, align: Align2) {
    let mut job = egui::text::LayoutJob::simple_singleline(label.to_owned(), font, color);
    job.wrap.max_width = rect.width().max(0.0);
    job.wrap.max_rows = 1;
    job.wrap.overflow_character = Some('…');
    let galley = ui.fonts_mut(|f| f.layout_job(job));
    let at = align.anchor_size(rect.min + vec2(rect.width() * align.x().to_factor(), rect.height() * align.y().to_factor()), galley.size()).min;
    ui.painter_at(rect).galley(at, galley, color);
}

fn action(ui: &mut Ui, label: &str, icon: &str, primary: bool) -> Response {
    let t = Tokens::get(ui.ctx());
    let font = theme::semibold(13.0);
    let w = ui.fonts_mut(|f| f.layout_no_wrap(label.to_owned(), font.clone(), t.text).size().x) + 52.0;
    let (rect, response) = ui.allocate_exact_size(vec2(w, 36.0), Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label));
    ui.painter().rect_filled(
        rect,
        CornerRadius::same(3),
        if primary {
            if response.hovered() { t.accent_strong } else { t.accent }
        } else if response.hovered() {
            t.hover
        } else {
            t.button
        },
    );
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(3),
        Stroke::new(1.0, if response.has_focus() { t.accent } else { t.border }),
        StrokeKind::Inside,
    );
    let foreground = if primary { Color32::WHITE } else { t.text };
    icons::paint(ui, icon, Rect::from_center_size(pos2(rect.left() + 19.0, rect.center().y), vec2(16.0, 16.0)), foreground);
    text(ui, Rect::from_min_max(rect.min + vec2(34.0, 0.0), rect.max - vec2(10.0, 0.0)), label, font, foreground, Align2::LEFT_CENTER);
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn heading(ui: &mut Ui, label: &str) {
    ui.label(egui::RichText::new(label.to_uppercase()).font(theme::semibold(10.0)).color(Tokens::get(ui.ctx()).text_dim));
    ui.add_space(12.0);
}

// Home's Web preset is disambiguated from the upstream workspace called Web.
// Keep the usual tl! fallback for catalog entries without a Home-specific translation.
fn preset_label(source: &str) -> &str {
    let translated = crate::i18n::tr_ctx(crate::i18n::current(), "varos-home", source);
    if translated == source { tl!(source) } else { translated }
}

fn presets(app: &mut VectorcraftApp, ui: &mut Ui) {
    heading(ui, tl!("Start with"));
    let cards = [
        (tl!("A4"), "210 × 297 mm", json!({"preset":"A4","units":"mm"}), 210.0_f32 / 297.0),
        (tl!("Post"), "1080 × 1080 px", json!({"preset":"Social Square Post 1080×1080"}), 1.0),
        (tl!("Story"), "1080 × 1920 px", json!({"preset":"Social Story 1080×1920"}), 1080.0 / 1920.0),
        (preset_label("Web"), "1440 × 900 px", json!({"preset":"Web 1440×900"}), 1.6),
        (tl!("Poster"), "50 × 70 cm", json!({"width":"50 cm","height":"70 cm","units":"cm"}), 5.0 / 7.0),
        (tl!("Icon"), "1024 × 1024 px", json!({"width":1024,"height":1024,"units":"px"}), 1.0),
    ];
    let width = ((ui.available_width() - 20.0) / 3.0).max(80.0);
    for row in cards.chunks(3) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            for (label, size, params, aspect) in row {
                let t = Tokens::get(ui.ctx());
                let (r, response) = ui.allocate_exact_size(vec2(width, 140.0), Sense::click());
                response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), *label));
                ui.painter().rect_filled(r, CornerRadius::same(8), if response.hovered() { t.hover } else { t.panel });
                ui.painter().rect_stroke(
                    r,
                    CornerRadius::same(8),
                    Stroke::new(1.0, if response.has_focus() { t.accent } else { t.border }),
                    StrokeKind::Inside,
                );
                let preview = vec2(48.0 * aspect.min(1.0), 48.0 / aspect.max(1.0));
                let page = Rect::from_center_size(pos2(r.center().x, r.top() + 43.0), preview);
                ui.painter().rect_filled(page, CornerRadius::same(3), t.text_dim);
                text(
                    ui,
                    Rect::from_min_max(r.min + vec2(10.0, 83.0), pos2(r.right() - 10.0, r.top() + 106.0)),
                    label,
                    theme::semibold(12.0),
                    t.text,
                    Align2::LEFT_CENTER,
                );
                text(
                    ui,
                    Rect::from_min_max(r.min + vec2(10.0, 106.0), r.max - vec2(8.0, 10.0)),
                    size,
                    egui::FontId::proportional(10.5),
                    t.text_dim,
                    Align2::LEFT_CENTER,
                );
                if response.on_hover_text(*size).clicked() {
                    run(app, "file.new", params.clone());
                }
            }
        });
        ui.add_space(6.0);
    }
}

/// Two folder levels provide context; the user's home is always abbreviated.
fn folder(path: &str) -> String {
    folder_in(path, std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).ok().filter(|h| !h.is_empty()).as_deref())
}

fn folder_in(path: &str, home: Option<&str>) -> String {
    let path = path.replace('\\', "/");
    let parent = path.rsplit_once('/').map_or("", |(p, _)| p).trim_end_matches('/');
    let home = home.map(|p| p.replace('\\', "/"));
    if let Some(home) = home.as_deref() {
        if parent == home {
            return "~".into();
        }
        if let Some(relative) = parent.strip_prefix(home).and_then(|p| p.strip_prefix('/')) {
            let parts: Vec<_> = relative.split('/').filter(|p| !p.is_empty()).collect();
            return if parts.len() < 2 {
                format!("~ › {relative}")
            } else {
                parts.into_iter().rev().take(2).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join(" › ")
            };
        }
    }
    let parts: Vec<_> = parent.split('/').filter(|p| !p.is_empty()).rev().take(2).collect();
    parts.into_iter().rev().collect::<Vec<_>>().join(" › ")
}

fn recent_modified(path: &str) -> Option<i64> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::fs::metadata(path).ok()?.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs().try_into().ok()
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = path;
        None
    } // Browser recents expose no filesystem timestamps.
}

/// Only list changes touch the filesystem, never a repaint of the same Home list.
fn recent_dates(ctx: &egui::Context, files: &[String]) -> Vec<Option<i64>> {
    type Cache = (Vec<String>, Vec<Option<i64>>);
    let id = egui::Id::new("home-recent-dates");
    if let Some((key, dates)) = ctx.data(|d| d.get_temp::<Cache>(id))
        && key == files
    {
        return dates;
    }
    let dates: Vec<_> = files.iter().map(|p| recent_modified(p)).collect();
    ctx.data_mut(|d| d.insert_temp(id, (files.to_vec(), dates.clone())));
    dates
}

fn recent_date(modified: Option<i64>, now: Option<i64>) -> String {
    let (Some(at), Some(now)) = (modified, now) else { return String::new() };
    let age = now.saturating_sub(at).max(0);
    if age < 3600 {
        return crate::i18n::fmt(tl!("{minutes} min ago"), &[("minutes", &(age / 60).to_string())]);
    }
    if age < 86400 && now.div_euclid(86400) == at.div_euclid(86400) {
        return crate::i18n::fmt(tl!("{hours} h ago"), &[("hours", &(age / 3600).to_string())]);
    }
    if now.div_euclid(86400) - at.div_euclid(86400) == 1 {
        return tl!("Yesterday").into();
    }
    let [_, month, day, ..] = vectorcraft_doc::metadata::civil(at);
    let months = [
        tl!("Jan"),
        tl!("Feb"),
        tl!("Mar"),
        tl!("Apr"),
        tl!("May"),
        tl!("Jun"),
        tl!("Jul"),
        tl!("Aug"),
        tl!("Sep"),
        tl!("Oct"),
        tl!("Nov"),
        tl!("Dec"),
    ];
    format!("{} {day}", months.get(month.saturating_sub(1) as usize).copied().unwrap_or(""))
}

fn recent(app: &mut VectorcraftApp, ui: &mut Ui) {
    let files: Vec<_> = crate::io::recent_files(app).iter().take(6).cloned().collect();
    let dates = recent_dates(ui.ctx(), &files);
    if files.is_empty() {
        return;
    }
    let now = vectorcraft_doc::metadata::now_unix();
    heading(ui, tl!("Recent"));
    ui.spacing_mut().item_spacing.y = 0.0;
    for (path, modified) in files.into_iter().zip(dates) {
        let t = Tokens::get(ui.ctx());
        let name = path.rsplit(['/', '\\']).next().unwrap_or(&path);
        let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::click());
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), name));
        if response.hovered() || response.has_focus() {
            ui.painter().rect_filled(rect, CornerRadius::same(3), t.hover);
        }
        if response.has_focus() {
            ui.painter().rect_stroke(rect, CornerRadius::same(3), Stroke::new(1.0, t.accent), StrokeKind::Inside);
        }
        let chip = Rect::from_center_size(pos2(rect.left() + 19.0, rect.center().y), vec2(30.0, 22.0));
        ui.painter().rect_filled(chip, CornerRadius::same(3), t.panel);
        let page = Rect::from_center_size(chip.center(), vec2(10.0, 14.0));
        ui.painter().rect_filled(page, CornerRadius::same(1), t.text_dim);
        let date = recent_date(modified, now);
        let date_width = if date.is_empty() { 8.0 } else { 82.0 };
        text(
            ui,
            Rect::from_min_max(rect.min + vec2(44.0, 1.0), pos2(rect.right() - date_width, rect.top() + 18.0)),
            name,
            theme::semibold(11.0),
            t.text,
            Align2::LEFT_CENTER,
        );
        text(
            ui,
            Rect::from_min_max(rect.min + vec2(44.0, 18.0), pos2(rect.right() - date_width, rect.bottom() - 1.0)),
            &folder(&path),
            egui::FontId::proportional(10.0),
            t.text_dim,
            Align2::LEFT_CENTER,
        );
        text(
            ui,
            Rect::from_min_max(pos2(rect.right() - 80.0, rect.top()), rect.max - vec2(6.0, 0.0)),
            &date,
            egui::FontId::proportional(10.0),
            t.text_dim,
            Align2::RIGHT_CENTER,
        );
        if response.on_hover_text(&path).clicked() {
            // The existing service surfaces failures and restores Home after a failed open.
            let _ = crate::io::open_reporting(app, &path);
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use serde_json::json;

    /// Exercise the real window layout, with fresh egui memory so old PanelStates cannot
    /// masquerade as visible chrome. This never creates a native window or render worker.
    pub(crate) fn assert_chrome(app: &mut VectorcraftApp, document: bool) {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx, crate::theme::Brightness::Dark);
        egui_extras::install_image_loaders(&ctx);
        app.fonts_ready = true;
        app.canvas.worker_started = true;
        app.canvas_rect = None;
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 900.0))),
            max_texture_side: Some(8192),
            ..Default::default()
        };
        let mut out = ctx.run_ui(raw, |ui| app.ui(ui));
        out.textures_delta.clear();
        assert!(!app.ui.status.starts_with("Internal error:"), "{}", app.ui.status);
        let panel = |id| egui::containers::panel::PanelState::load(&ctx, egui::Id::new(id)).is_some();
        assert!(panel("app_bar"), "title bar remains available");
        for id in ["toolbar", "dock", "icon_column", "control_bar", "status_bar", "hint_bar"] {
            assert_eq!(panel(id), document, "{id}: editor chrome follows the Home gate");
        }
        assert_eq!(app.canvas_rect.is_some(), document);
        let texts: Vec<_> = out
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Text(t) => Some(t.galley.text()),
                _ => None,
            })
            .collect();
        assert_eq!(texts.contains(&"Welcome to Varos"), !document);
        if !document {
            for label in ["New file", "Open", "START WITH"] {
                assert!(texts.contains(&label), "Home keeps {label}: {texts:?}");
            }
            assert!(!texts.contains(&"File"), "in-window editor menus stay hidden");
        }
    }

    #[test]
    fn home_keeps_cmd_n_and_cmd_o_shortcuts() {
        for key in [egui::Key::N, egui::Key::O] {
            let opened = std::rc::Rc::new(std::cell::Cell::new(false));
            let called = opened.clone();
            let services = crate::Services { open_async: Some(Box::new(move || called.set(true))), ..Default::default() };
            let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), services);
            assert!(is_home(&app));
            let ctx = egui::Context::default();
            let modifiers = egui::Modifiers::COMMAND;
            let raw = egui::RawInput {
                events: vec![
                    egui::Event::ModifiersChanged(modifiers),
                    egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers },
                ],
                ..Default::default()
            };
            let mut out = ctx.run_ui(raw, |ui| crate::shortcuts::handle(&mut app, ui.ctx()));
            out.textures_delta.clear();
            if key == egui::Key::N {
                assert_eq!(app.ui.dialog.as_ref().map(|d| d.kind.as_str()), Some("newDocument"));
            } else {
                assert!(opened.get(), "Cmd+O reaches the open service on Home");
            }
        }
    }

    #[test]
    fn home_chrome_returns_on_the_first_frame_after_new_and_close() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        app.ui.control_bar = true;
        assert_chrome(&mut app, false);
        app.run("file.new", json!({})).unwrap();
        assert_chrome(&mut app, true);
        app.run("app.home", json!({})).unwrap();
        assert_chrome(&mut app, false);
        app.run("file.new", json!({})).unwrap();
        // No intervening canvas frame: even a stale Home key cannot hide document chrome.
        assert_chrome(&mut app, true);
        app.run("file.close", json!({})).unwrap();
        app.run("file.close", json!({})).unwrap();
        assert_chrome(&mut app, false);
    }
}

#[cfg(test)]
mod board_tests {
    use super::*;

    fn frame(app: &mut VectorcraftApp, ctx: &egui::Context, events: Vec<egui::Event>) -> egui::FullOutput {
        let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1000.0, 800.0))), events, ..Default::default() };
        let mut out = ctx.run_ui(raw, |ui| crate::canvas::show(app, ui));
        out.textures_delta.clear(); // No GPU in this layout test.
        out
    }
    fn shown(out: &egui::FullOutput, label: &str) -> Option<Rect> {
        out.shapes.iter().find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text() == label => Some(t.visual_bounding_rect()),
            _ => None,
        })
    }
    #[test]
    fn home_new_board_click_opens_a_free_canvas_with_editor_chrome() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx, crate::theme::Brightness::Dark);
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let out = frame(&mut app, &ctx, vec![]);
        let at = shown(&out, "New board").expect("third primary action").center();
        let button = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(at), button(true)]);
        frame(&mut app, &ctx, vec![button(false)]);
        assert_eq!(app.session.documents().len(), 1);
        assert!(app.session.active().unwrap().doc.artboards.is_empty());
        assert!(!is_home(&app));
        app.ui.control_bar = true;
        tests::assert_chrome(&mut app, true);
    }
    #[test]
    fn board_fit_uses_visible_art_and_empty_board_uses_the_ruler_origin() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        app.run("file.newBoard", json!({})).unwrap();
        app.canvas_rect = Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1000.0, 800.0)));
        app.run("view.fitArtboard", json!({})).unwrap();
        assert_eq!(app.view().unwrap().center, vectorcraft_geom::Point::ZERO);
        app.run("shape.rectangle", json!({"x":-300,"y":-200,"width":120,"height":80})).unwrap();
        let target = vectorcraft_render::encode::art_bounds(&app.session.active().unwrap().doc).unwrap();
        app.run("view.fitArtboard", json!({})).unwrap();
        let v = app.view().unwrap();
        assert_eq!(v.center, target.center());
        assert!(v.zoom.is_finite() && v.zoom > 1.0 && v.fitted);
        assert!(app.session.active().unwrap().doc.artboards.is_empty());
        assert_eq!(app.ruler_origin(), vectorcraft_geom::Point::ZERO);
    }
    #[test]
    fn each_home_preset_click_creates_its_stated_size_and_units() {
        use vectorcraft_doc::Unit;
        for (label, width, height, unit) in [
            ("A4", 210.0 * 72.0 / 25.4, 297.0 * 72.0 / 25.4, Unit::Millimeters),
            ("Post", 1080.0, 1080.0, Unit::Pixels),
            ("Story", 1080.0, 1920.0, Unit::Pixels),
            ("Web", 1440.0, 900.0, Unit::Pixels),
            ("Poster", 50.0 * 72.0 / 2.54, 70.0 * 72.0 / 2.54, Unit::Centimeters),
            ("Icon", 1024.0, 1024.0, Unit::Pixels),
        ] {
            let ctx = egui::Context::default();
            crate::theme::install_fonts(&ctx);
            let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
            let out = frame(&mut app, &ctx, vec![]);
            let at = shown(&out, label).unwrap().center();
            let button =
                |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
            frame(&mut app, &ctx, vec![egui::Event::PointerMoved(at), button(true)]);
            frame(&mut app, &ctx, vec![button(false)]);
            let doc = &app.session.active().unwrap().doc;
            assert_eq!(doc.units, unit, "{label}");
            let r = doc.artboards[0].rect;
            assert!((r.width() - width).abs() < 0.01 && (r.height() - height).abs() < 0.01, "{label}: {r:?}");
        }
    }

    #[test]
    fn arabic_subtitle_is_shaped_present_and_right_aligned() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let out = frame(&mut app, &ctx, vec![]);
        let tex = ctx.data(|d| d.get_temp::<egui::TextureHandle>(egui::Id::new(("home-arabic-greeting", 40_u32)))).unwrap();
        let mesh = out
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Mesh(m) if m.texture_id == tex.id() => Some(m),
                _ => None,
            })
            .expect("Arabic subtitle texture is painted");
        assert!(tex.size()[0] > 0);
        // The 1000 px canvas has a 24 px Home inset on the right.
        let right = mesh.vertices.iter().map(|v| v.pos.x).fold(f32::NEG_INFINITY, f32::max);
        assert!((right - 976.0).abs() < 1.0, "right edge: {right}");
        assert_eq!(ARABIC_GREETING, "أهلاً بك في فاروس");
    }

    #[test]
    fn recent_dates_are_relative_cached_and_full_path_is_in_tooltip() {
        let now = 1792411200; // 2026-10-19 12:00 UTC.
        assert_eq!(recent_date(Some(now - 7200), Some(now)), "2 h ago");
        assert_eq!(recent_date(Some(now - 86400), Some(now)), "Yesterday");
        assert_eq!(recent_date(Some(now - 5 * 86400), Some(now)), "Oct 14");
        assert_eq!(recent_date(None, Some(now)), "");
        assert_eq!(folder_in("/Users/test/file.svg", Some("/Users/test")), "~");
        assert_eq!(folder_in("/Users/test/Documents/Clients/file.svg", Some("/Users/test")), "Documents › Clients");
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let path = std::env::temp_dir().join(format!("home-date-{}.svg", std::process::id()));
        std::fs::write(&path, "date cache test").unwrap();
        let path = path.to_string_lossy().into_owned();
        let files = vec![path.clone()];
        let first = recent_dates(&ctx, &files);
        assert!(first[0].is_some());
        std::fs::remove_file(&path).unwrap();
        assert_eq!(recent_dates(&ctx, &files), first, "same list must not stat again");
        assert!(recent_dates(&ctx, &[]).is_empty());
        assert_eq!(recent_dates(&ctx, &files), vec![None], "changed list refreshes");
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        app.ui.recent_files = vec![path.clone()];
        ctx.all_styles_mut(|s| {
            s.interaction.tooltip_delay = 0.0;
            s.interaction.tooltip_grace_time = 0.0;
            s.interaction.show_tooltips_only_when_still = false;
        });
        let current = vectorcraft_doc::metadata::now_unix().unwrap();
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("home-recent-dates"), (vec![path.clone()], vec![Some(current - 120)])));
        let out = frame(&mut app, &ctx, vec![]);
        assert!(shown(&out, "2 min ago").is_some(), "relative date painted in the recent row");
        let at = shown(&out, path.rsplit('/').next().unwrap()).unwrap().center();
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(at)]);
        let out = frame(&mut app, &ctx, vec![]);
        assert!(shown(&out, &path).is_some(), "full path tooltip");
    }

    #[test]
    fn home_cards_hover_neutrally_and_folder_names_are_short() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx, crate::theme::Brightness::Dark);
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let out = frame(&mut app, &ctx, vec![]);
        let at = shown(&out, "Post").unwrap().center() - vec2(0.0, 45.0);
        let out = frame(&mut app, &ctx, vec![egui::Event::PointerMoved(at)]);
        let t = Tokens::get(&ctx);
        assert!(out.shapes.iter().any(
            |s| matches!(&s.shape,egui::Shape::Rect(r) if r.rect.contains(at) && r.fill == t.hover && r.corner_radius == CornerRadius::same(8))
        ));
        assert_eq!(folder("/private/tmp/art-session/Poster.vectorcraft"), "tmp › art-session");
        assert_eq!(folder(r"C:\Users\Ahmed\Clients\Poster.vectorcraft"), "Ahmed › Clients");
        assert_eq!(folder("Poster.vectorcraft"), "");
    }
}

#[cfg(test)]
#[path = "home_capture.rs"]
pub(crate) mod capture;
