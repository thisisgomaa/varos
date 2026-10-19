//! Varos: concept A, Script first. State is owned by the upstream menu, so choices survive closing.
use egui::{Ui, vec2};
use vectorcraft_text::{FontClass, FontDb, arabic_fonts::ArabicStyle};

use super::arabic_fonts::{self, ArabicMenu, SampleMode};
use crate::{theme::Tokens, widgets};

fn chip(ui: &mut Ui, on: bool, text: &str) -> egui::Response {
    ui.selectable_label(on, egui::RichText::new(text).size(11.0))
}

/// Search retains upstream's field ID, focus, query storage and keyboard handling.
pub(crate) fn search(ui: &mut Ui, field: egui::Id, arabic: &mut ArabicMenu) -> String {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        // Leave the compact language switch room even when translations grow.
        let auto = tl!("Auto");
        let auto_width = ui.painter().layout_no_wrap(auto.into(), egui::FontId::proportional(11.0), Tokens::get(ui.ctx()).text).size().x + 16.0;
        let db = FontDb::global();
        let has_arabic = db.menu_family_list().iter().any(|f| db.may_set_arabic(f));
        let glyph_width = if has_arabic {
            32.0
        } else {
            ui.painter().layout_no_wrap(tl!("Arabic").into(), egui::FontId::proportional(11.0), Tokens::get(ui.ctx()).text).size().x + 12.0
        };
        let width = (ui.available_width() - auto_width - glyph_width - 40.0).max(40.0);
        let query = ui
            .allocate_ui_with_layout(vec2(width, 28.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                widgets::search_field(ui, field, tl!("Search"))
            })
            .inner;
        ui.spacing_mut().item_spacing.x = 1.0;
        if chip(ui, arabic.samples == SampleMode::Auto, auto).on_hover_text(tl!("Follow the selected text's script")).clicked() {
            arabic.samples = SampleMode::Auto;
        }
        if chip(ui, arabic.samples == SampleMode::Latin, "Aa").on_hover_text(tl!("Latin samples")).clicked() {
            arabic.samples = SampleMode::Latin;
        }
        let response = shaped_chip(ui, arabic.samples == SampleMode::Arabic, "أب", tl!("Arabic"), None)
            .on_hover_text(tl!("Arabic samples in Arabic-capable fonts"));
        if response.clicked() {
            arabic.samples = SampleMode::Arabic;
        }
        query
    })
    .inner
}

/// Script indices 1/2/3 preserve upstream's Japanese/Latin/Thai matching exactly.
pub(crate) fn filters(ui: &mut Ui, script: &mut usize, class: &mut Option<FontClass>, favorites: &mut bool, arabic: &mut ArabicMenu) -> bool {
    let before = (*script, *class, *favorites, arabic.key());
    let labels = [(0, tl!("All")), (4, tl!("Arabic")), (2, tl!("Latin")), (1, tl!("Japanese")), (3, tl!("Thai"))];
    let available = ui.available_width();
    let widths: Vec<f32> = labels
        .iter()
        .map(|(_, label)| ui.painter().layout_no_wrap((*label).into(), egui::FontId::proportional(11.0), Tokens::get(ui.ctx()).text).size().x + 12.0)
        .collect();
    let total: f32 = widths.iter().sum();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for ((value, label), width) in labels.into_iter().zip(widths) {
            if ui
                .add_sized(
                    vec2(width + (available - total).max(0.0) / 5.0, 24.0),
                    egui::Button::selectable(*script == value, egui::RichText::new(label).size(11.0)),
                )
                .clicked()
            {
                *script = value;
            }
        }
    });
    arabic.only = *script == 4;
    if arabic.only {
        styles(ui, arabic);
    }
    egui::ScrollArea::horizontal().id_salt("varos-font-kinds").auto_shrink([false, true]).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            if chip(ui, *favorites, "★").on_hover_text(tl!("Favorites")).clicked() {
                *favorites = !*favorites;
            }
            if chip(ui, class.is_none(), tl!("Any Kind")).clicked() {
                *class = None;
            }
            for (kind, label) in [
                (FontClass::Serif, tl!("Serif")),
                (FontClass::Sans, tl!("Sans")),
                (FontClass::Rounded, tl!("Rounded")),
                (FontClass::Script, tl!("Script")),
                (FontClass::Monospaced, tl!("Mono")),
                (FontClass::Decorative, tl!("Decorative")),
            ] {
                if chip(ui, *class == Some(kind), label).clicked() {
                    *class = if *class == Some(kind) { None } else { Some(kind) };
                }
            }
        });
    });
    before != (*script, *class, *favorites, arabic.key())
}

fn styles(ui: &mut Ui, arabic: &mut ArabicMenu) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(4.0, 2.0);
        if chip(ui, arabic.style.is_none(), tl!("All Arabic")).clicked() {
            arabic.style = None;
        }
        for (style, name) in [
            (ArabicStyle::Naskh, "نسخ"),
            (ArabicStyle::Kufi, "كوفي"),
            (ArabicStyle::Ruqaa, "رقعة"),
            (ArabicStyle::Diwani, "ديواني"),
            (ArabicStyle::Thuluth, "ثلث"),
            (ArabicStyle::ModernSans, "حديث"),
        ] {
            if shaped_chip(ui, arabic.style == Some(style), name, arabic_fonts::style_label(style), Some(style))
                .on_hover_text(arabic_fonts::style_label(style))
                .on_hover_text(tl!("Guessed from the font's name; fonts it can't place are under All Arabic"))
                .clicked()
            {
                arabic.style = if arabic.style == Some(style) { None } else { Some(style) };
            }
        }
    });
}

/// Shape through the existing bounded background renderer. It also owns texture lifetimes and
/// graphics-context resets. Missing/unloaded style faces use their translated Latin label.
fn shaped_chip(ui: &mut Ui, on: bool, text: &str, fallback: &str, style: Option<ArabicStyle>) -> egui::Response {
    shaped_chip_in(ui, on, text, fallback, style, FontDb::global())
}

fn shaped_chip_in(ui: &mut Ui, on: bool, text: &str, fallback: &str, style: Option<ArabicStyle>, db: &FontDb) -> egui::Response {
    let scale = ui.ctx().pixels_per_point();
    let families = db.menu_family_list();
    let mut eligible = families.iter().filter(|f| db.may_set_arabic(f) && style.is_none_or(|s| db.arabic_style(f) == s)).peekable();
    let has_family = eligible.peek().is_some();
    let texture = eligible.find_map(|f| crate::font_menu::sample_texture(ui.ctx(), f, None, Some(text), 18.0));
    if let Some(texture) = texture {
        let size = texture.size_vec2() / scale;
        let button_size = if style.is_none() { vec2(32.0, 24.0) } else { size + vec2(12.0, 4.0) };
        let response = ui.add(egui::Button::selectable(on, "").min_size(button_size));
        let tint = if on { ui.visuals().selection.stroke.color } else { Tokens::get(ui.ctx()).text };
        let rect = egui::Rect::from_center_size(response.rect.center(), size);
        ui.painter().image(texture.id(), rect, egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)), tint);
        response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, on, fallback));
        response
    } else if style.is_none() && has_family {
        // Keep the glyph segment's width stable while its texture is being prepared.
        let response = ui.add(egui::Button::selectable(on, "").min_size(vec2(32.0, 24.0)));
        response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, on, fallback));
        response
    } else {
        chip(ui, on, fallback)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_segment_has_a_label_without_arabic_fonts() {
        let db = FontDb::with_font_dirs(vec![]);
        db.set_system_fallback(false);
        let ctx = egui::Context::default();
        let mut out = ctx.run_ui(Default::default(), |ui| {
            let r = shaped_chip_in(ui, false, "أب", "Arabic", None, &db);
            assert!(r.rect.width() > 20.0);
        });
        out.textures_delta.clear();
        assert!(out.shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == "Arabic")));
    }

    #[test]
    fn sample_switch_overrides_arabic_selection_and_survives_state_round_trip() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let id = egui::Id::new("sample-language-test");
        let state = egui::Id::new("sample-language-state");
        let db = FontDb::with_font_dirs(vec![]);
        db.set_system_fallback(false);
        db.add_font(include_bytes!("../../../text-arabic/fixtures/NotoSansArabic.ttf").to_vec());
        let frame = |events| {
            let mut menu: ArabicMenu = ctx.data(|d| d.get_temp(state)).unwrap_or_default();
            let mut shapes = vec![];
            let mut out = ctx.run_ui(
                egui::RawInput { events, screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(400.0, 140.0))), ..Default::default() },
                |ui| {
                    search(ui, id, &mut menu);
                },
            );
            for shape in &out.shapes {
                if let egui::Shape::Text(t) = &shape.shape {
                    shapes.push((t.galley.text().to_string(), t.visual_bounding_rect()));
                }
            }
            out.textures_delta.clear();
            ctx.data_mut(|d| d.insert_temp(state, menu.clone()));
            (menu, shapes, out.shapes)
        };
        let click = |pos| {
            let event = |pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
            frame(vec![egui::Event::PointerMoved(pos), event(true)]);
            frame(vec![event(false)])
        };
        let (auto, texts, _) = frame(vec![]);
        assert_eq!(auto.samples, SampleMode::Auto);
        assert_eq!(auto.row_sample(&db, "Noto Sans Arabic", Some("بسم")), Some((Some("بسم".into()), true)));
        let aa = texts.iter().find(|(s, _)| s == "Aa").unwrap().1.center();
        let (latin, _, _) = click(aa);
        assert_eq!(latin.samples, SampleMode::Latin);
        assert_eq!(latin.row_sample(&db, "Noto Sans Arabic", Some("بسم")), Some((None, false)));
        for text in ["Hello", "雅楽", "ภาษาไทย", "Привет"] {
            assert_eq!(latin.row_sample(&db, "Noto Sans Arabic", Some(text)), Some((None, false)), "Aa stays Latin for {text}");
        }
        // The third segment is immediately to the right of Aa (its label is a shaped texture).
        let (arabic, _, _) = click(aa + vec2(30.0, 0.0));
        assert_eq!(arabic.samples, SampleMode::Arabic);
        assert_eq!(
            arabic.row_sample(&db, "Noto Sans Arabic", Some("Hello")),
            Some((Some(vectorcraft_text::arabic_fonts::ARABIC_SAMPLE.into()), true))
        );
        assert_eq!(arabic.row_sample(&db, "Source Serif 4", Some("Hello")), Some((Some("Hello".into()), false)));
        assert_eq!(frame(vec![]).0.samples, SampleMode::Arabic, "egui menu state round trip");
        let (_, texts, _) = frame(vec![]);
        let auto = texts.iter().find(|(s, _)| s == "Auto").unwrap().1.center();
        assert_eq!(click(auto).0.samples, SampleMode::Auto);
    }
}

/// Opt-in visual evidence of the real widgets, separate from the portable headless checks.
#[cfg(test)]
#[test]
#[ignore = "manual screenshot evidence; requires a graphics adapter"]
fn render_concept_a_evidence() {
    use egui_kittest::Harness;
    let mut installed = false;
    let mut h = Harness::builder().with_size(vec2(376.0, 280.0)).build_ui_state(
        move |ui, state: &mut (usize, Option<FontClass>, bool, ArabicMenu)| {
            if !std::mem::replace(&mut installed, true) {
                crate::theme::install_fonts(ui.ctx());
                crate::theme::apply(ui.ctx(), crate::theme::Brightness::Dark);
                return;
            }
            ui.set_width(360.0);
            search(ui, egui::Id::new("evidence-query"), &mut state.3);
            filters(ui, &mut state.0, &mut state.1, &mut state.2, &mut state.3);
        },
        (0, None, false, ArabicMenu::default()),
    );
    h.run_steps(5);
    for _ in 0..4 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        h.run_steps(2);
    }
    let dir = std::env::var("VAROS_FONTMENU_EVIDENCE").expect("set VAROS_FONTMENU_EVIDENCE to a writable evidence directory");
    std::fs::create_dir_all(&dir).unwrap();
    h.render().expect("headless graphics adapter").save(format!("{dir}/all.png")).unwrap();
    h.state_mut().0 = 4;
    h.run_steps(5);
    // Background samples need wall time; a few bounded waits, only in this opt-in evidence run.
    for _ in 0..4 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        h.run_steps(2);
    }
    h.render().unwrap().save(format!("{dir}/arabic.png")).unwrap();
    h.state_mut().0 = 2;
    h.run_steps(3);
    h.render().unwrap().save(format!("{dir}/latin.png")).unwrap();
}
