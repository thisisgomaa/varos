//! Varos: concept A, Script first. State is owned by the upstream menu, so choices survive closing.
use std::sync::Arc;

use egui::{Ui, vec2};
use vectorcraft_text::{FontClass, FontDb, arabic_fonts::ArabicStyle};

use super::arabic_fonts::{self, ArabicMenu, SampleMode};
use crate::{theme::Tokens, widgets};

/// Varos-only script index; upstream keeps indices 0–3 for all/Japanese/Latin/Thai.
pub(crate) const ARABIC_SCRIPT: usize = 4;

/// Each shaped label owns a candidate cursor. Pending work keeps the cursor still; only a
/// completed unavailable render advances it. Families are classified once per font generation.
#[derive(Clone, Default)]
struct Candidates {
    families: Arc<[String]>,
    next: usize,
}

#[derive(Clone)]
struct HeaderFamilies {
    generation: u64,
    groups: [Candidates; 7],
}

impl HeaderFamilies {
    fn new(db: &FontDb) -> Self {
        // Read before scanning: a concurrent catalog update makes this stale next frame.
        let generation = db.generation();
        let mut groups: [Vec<String>; 7] = std::array::from_fn(|_| Vec::new());
        for family in db.menu_family_list().iter().filter(|f| db.may_set_arabic(f)) {
            if let Some(all) = groups.first_mut() {
                all.push(family.clone());
            }
            let slot = Self::slot(Some(db.arabic_style(family)));
            if slot != 0
                && let Some(group) = groups.get_mut(slot)
            {
                group.push(family.clone());
            }
        }
        Self { generation, groups: groups.map(|families| Candidates { families: families.into(), next: 0 }) }
    }

    fn slot(style: Option<ArabicStyle>) -> usize {
        style.and_then(|s| ArabicStyle::ALL.iter().position(|candidate| *candidate == s)).map_or(0, |i| i + 1)
    }

    fn has_arabic(&self) -> bool {
        self.groups.first().is_some_and(|g| !g.families.is_empty())
    }

    fn family(&self, style: Option<ArabicStyle>) -> Option<&str> {
        let group = self.groups.get(Self::slot(style))?;
        group.families.get(group.next).map(String::as_str)
    }

    fn advance(&mut self, style: Option<ArabicStyle>, family: &str) {
        if let Some(group) = self.groups.get_mut(Self::slot(style))
            && group.families.get(group.next).is_some_and(|f| f == family)
        {
            group.next = group.next.saturating_add(1);
        }
    }
}

fn header_families<R>(ctx: &egui::Context, db: &FontDb, read: impl FnOnce(&mut HeaderFamilies) -> R) -> R {
    // Test databases share no catalog even if their generation counters happen to match.
    let id = egui::Id::new(("varos-font-header-families", std::ptr::from_ref(db) as usize));
    ctx.data_mut(|d| {
        let cache = d.get_temp_mut_or_insert_with(id, || HeaderFamilies::new(db));
        if cache.generation != db.generation() {
            *cache = HeaderFamilies::new(db);
        }
        read(cache)
    })
}

const CHIP_HEIGHT: f32 = 24.0;
const SHAPED_HEIGHT: f32 = 18.0;
// Reserve the same shaped-name slot before/after rendering, failure, or texture reset.
const STYLE_NAME_WIDTH: f32 = 52.0;

fn chip(ui: &mut Ui, on: bool, text: &str) -> egui::Response {
    ui.selectable_label(on, egui::RichText::new(text).size(11.0))
}

/// Search retains upstream's field ID, focus, query storage and keyboard handling.
pub(crate) fn search(ui: &mut Ui, field: egui::Id, arabic: &mut ArabicMenu, samples: bool) -> String {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        if !samples {
            return widgets::search_field(ui, field, tl!("Search"));
        }
        // Leave the compact language switch room even when translations grow.
        let auto = tl!("Auto");
        let auto_width = ui.painter().layout_no_wrap(auto.into(), egui::FontId::proportional(11.0), Tokens::get(ui.ctx()).text).size().x + 16.0;
        let db = FontDb::global();
        let has_arabic = header_families(ui.ctx(), db, |cache| cache.has_arabic());
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
        if chip(ui, arabic.samples == SampleMode::Auto, auto)
            .on_hover_text(tl!("Selected Arabic text, otherwise Arabic samples in Arabic-capable fonts"))
            .clicked()
        {
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
    let labels = [(0, tl!("All Scripts")), (ARABIC_SCRIPT, tl!("Arabic")), (2, tl!("Latin")), (1, tl!("Japanese")), (3, tl!("Thai"))];
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
    arabic.only = *script == ARABIC_SCRIPT;
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
                (FontClass::Monospaced, tl!("Monospaced")),
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
        if ui
            .add(egui::Button::selectable(arabic.style.is_none(), egui::RichText::new(tl!("All Arabic")).size(11.0)).min_size(vec2(0.0, CHIP_HEIGHT)))
            .clicked()
        {
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

/// Shape through the existing bounded background renderer. Texture lifetime and reset handling
/// remain upstream's. Style chips always keep their translated label beside the shaped name.
fn shaped_chip(ui: &mut Ui, on: bool, text: &str, fallback: &str, style: Option<ArabicStyle>) -> egui::Response {
    shaped_chip_in(ui, on, text, fallback, style, FontDb::global())
}

/// Ask exactly one candidate per call. Only a cached unavailable result permits advancing.
fn chip_texture(
    ctx: &egui::Context,
    db: &FontDb,
    style: Option<ArabicStyle>,
    render: impl FnOnce(&str) -> Option<Option<egui::TextureHandle>>,
) -> Option<egui::TextureHandle> {
    let family = header_families(ctx, db, |cache| cache.family(style).map(str::to_owned))?;
    match render(&family) {
        Some(Some(texture)) => Some(texture),
        Some(None) => {
            header_families(ctx, db, |cache| cache.advance(style, &family));
            ctx.request_repaint();
            None
        }
        None => None, // Rendering or budget-limited: keep this family.
    }
}

fn shaped_chip_in(ui: &mut Ui, on: bool, text: &str, fallback: &str, style: Option<ArabicStyle>, db: &FontDb) -> egui::Response {
    let has_family = header_families(ui.ctx(), db, |cache| cache.has_arabic());
    let texture =
        chip_texture(ui.ctx(), db, style, |family| crate::font_menu::sample_texture_result(ui.ctx(), family, None, Some(text), SHAPED_HEIGHT));
    paint_shaped_chip(ui, on, fallback, style, has_family, texture)
}

fn paint_shaped_chip(
    ui: &mut Ui,
    on: bool,
    fallback: &str,
    style: Option<ArabicStyle>,
    has_family: bool,
    texture: Option<egui::TextureHandle>,
) -> egui::Response {
    let tint = if on { ui.visuals().selection.stroke.color } else { Tokens::get(ui.ctx()).text };
    let label = ui.painter().layout_no_wrap(fallback.into(), egui::FontId::proportional(11.0), tint);
    let show_label = style.is_some() || !has_family;
    let width = if style.is_some() {
        label.size().x + 6.0 + STYLE_NAME_WIDTH + 12.0
    } else if has_family {
        32.0
    } else {
        label.size().x + 12.0
    };
    let response = ui.add_sized(vec2(width, CHIP_HEIGHT), egui::Button::selectable(on, ""));
    if show_label {
        ui.painter().galley(response.rect.left_center() + vec2(6.0, -label.size().y * 0.5), label, tint);
    }
    if let Some(texture) = texture {
        let natural = texture.size_vec2() / ui.ctx().pixels_per_point();
        let slot_width = if style.is_some() { STYLE_NAME_WIDTH } else { 20.0 };
        let factor = (slot_width / natural.x.max(1.0)).min(SHAPED_HEIGHT / natural.y.max(1.0)).min(1.0);
        let size = natural * factor;
        let center = if style.is_some() { response.rect.right_center() - vec2(6.0 + slot_width * 0.5, 0.0) } else { response.rect.center() };
        let rect = egui::Rect::from_center_size(center, size);
        ui.painter().image(texture.id(), rect, egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)), tint);
    }
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, on, fallback));
    response
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
                    search(ui, id, &mut menu, true);
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

    #[test]
    fn candidates_wait_for_pending_work_and_refresh_only_when_fonts_change() {
        let db = FontDb::with_font_dirs(vec![]);
        db.set_system_fallback(false);
        let ctx = egui::Context::default();
        assert!(!header_families(&ctx, &db, |cache| cache.has_arabic()));
        db.add_font(include_bytes!("../../../text-arabic/fixtures/NotoSansArabic.ttf").to_vec());
        db.add_font(include_bytes!("../../assets/varos/fonts/IBMPlexSansArabic-Regular.ttf").to_vec());
        let families = header_families(&ctx, &db, |cache| cache.groups.first().unwrap().families.clone());
        assert_eq!(families.len(), 2);
        let first = families.first().unwrap();
        for _ in 0..8 {
            let mut calls = 0;
            assert!(
                chip_texture(&ctx, &db, None, |family| {
                    calls += 1;
                    assert_eq!(family, first);
                    None // Pending or the per-frame budget is exhausted.
                })
                .is_none()
            );
            assert_eq!(calls, 1);
            assert!(header_families(&ctx, &db, |cache| Arc::ptr_eq(&cache.groups.first().unwrap().families, &families)));
        }
        chip_texture(&ctx, &db, None, |family| {
            assert_eq!(family, first);
            Some(None) // Completed and cached: try the second one next time, not now.
        });
        assert_eq!(header_families(&ctx, &db, |cache| cache.family(None).map(str::to_owned)).as_ref(), families.get(1));
        let tex = ctx.load_texture("test-chip", egui::ColorImage::filled([20, 18], egui::Color32::WHITE), Default::default());
        assert!(chip_texture(&ctx, &db, None, |_| Some(Some(tex))).is_some());
        assert_eq!(header_families(&ctx, &db, |cache| cache.family(None).map(str::to_owned)).as_ref(), families.get(1));
        chip_texture(&ctx, &db, None, |_| Some(None));
        assert!(header_families(&ctx, &db, |cache| cache.family(None).is_none()));
        assert!(header_families(&ctx, &db, |cache| cache.has_arabic()), "capability is independent of exhausted label candidates");
        // A newly added face invalidates the candidate cursor and the memoized capability.
        db.add_font(include_bytes!("../../assets/varos/fonts/Inter-Regular.ttf").to_vec());
        assert_eq!(header_families(&ctx, &db, |cache| cache.family(None).map(str::to_owned)).as_ref(), Some(first));
        for style in ArabicStyle::ALL {
            let expected = families.iter().find(|f| db.arabic_style(f) == style);
            assert_eq!(header_families(&ctx, &db, |cache| cache.family(Some(style)).map(str::to_owned)).as_ref(), expected);
        }
    }

    #[test]
    fn style_chip_labels_and_geometry_survive_pending_missing_and_reset_textures() {
        let ctx = egui::Context::default();
        let texture = ctx.load_texture("style-test", egui::ColorImage::filled([50, 18], egui::Color32::WHITE), Default::default());
        for style in ArabicStyle::ALL {
            let label = arabic_fonts::style_label(style);
            let mut sizes = vec![];
            for (has_family, tex) in [(true, None), (true, Some(texture.clone())), (true, None), (false, None)] {
                let mut out = ctx.run_ui(Default::default(), |ui| {
                    let chip = paint_shaped_chip(ui, false, label, Some(style), has_family, tex.clone());
                    sizes.push(chip.rect.size());
                });
                out.textures_delta.clear();
                assert!(out.shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == label)));
            }
            assert!(sizes.windows(2).all(|pair| pair[0] == pair[1]), "{label}: {sizes:?}");
            assert_eq!(sizes.first().unwrap().y, CHIP_HEIGHT);
        }
    }

    #[test]
    fn sample_switch_is_hidden_when_previews_are_off() {
        let ctx = egui::Context::default();
        let mut menu = ArabicMenu { samples: SampleMode::Arabic, ..Default::default() };
        let mut out = ctx.run_ui(Default::default(), |ui| {
            search(ui, egui::Id::new("no-previews"), &mut menu, false);
        });
        out.textures_delta.clear();
        assert!(!out.shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Text(t) if ["Auto", "Aa", "Arabic"].contains(&t.galley.text()))));
        assert_eq!(menu.samples, SampleMode::Arabic, "the hidden preference keeps the chosen mode");
    }

    /// Opt-in visual evidence of the real widgets, separate from the portable headless checks.
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
                search(ui, egui::Id::new("evidence-query"), &mut state.3, true);
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
        h.state_mut().0 = ARABIC_SCRIPT;
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
}
