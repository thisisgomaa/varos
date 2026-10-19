//! Varos: the Middle Eastern groups ([`super`]) and the Glyphs panel's Arabic subsets, driven
//! headless.

use serde_json::json;
use vectorcraft_color::Paint;
use vectorcraft_doc::{NodeKind, ParaDirection, TextObject};
use vectorcraft_engine::Session;

use super::{Drawn, Rows, character_rows, default_on, paragraph_rows};

/// The control a group drew as `key`.
trait Get {
    fn get(&self, key: &str) -> &Drawn;
}

impl Get for Rows {
    fn get(&self, key: &str) -> &Drawn {
        self.controls.iter().find(|c| c.key == key).unwrap_or_else(|| panic!("no control {key} in {:?}", self.controls))
    }
}
use crate::VectorcraftApp;

fn app_with(text: &str) -> (VectorcraftApp, u64) {
    let mut app = VectorcraftApp::new(Session::new(), Default::default());
    app.session.prefs.show_indic_options = true;
    app.run("file.new", json!({"width": 600, "height": 400})).unwrap();
    let id = app.run("text.create", json!({"x": 20, "y": 50, "text": text})).unwrap()["id"].as_u64().unwrap();
    app.session.execute("select.set", &json!({"ids": [id]})).unwrap();
    (app, id)
}

fn text(app: &VectorcraftApp, id: u64) -> TextObject {
    match &app.session.active().unwrap().doc.node(vectorcraft_doc::NodeId(id)).unwrap().kind {
        NodeKind::Text(t) => (**t).clone(),
        _ => panic!("text"),
    }
}

fn input(events: Vec<egui::Event>) -> egui::RawInput {
    egui::RawInput { events, screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(320.0, 400.0))), ..Default::default() }
}

fn button(pos: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() }
}

/// Draw `rows` in a frame with `events`; the rows drawn and the texts painted.
fn frame(
    ctx: &egui::Context,
    app: &mut VectorcraftApp,
    events: Vec<egui::Event>,
    rows: fn(&mut VectorcraftApp, &mut egui::Ui) -> Rows,
) -> (Rows, Vec<String>) {
    let mut out_rows = Rows::default();
    let mut out = ctx.run_ui(input(events), |ui| out_rows = rows(app, ui));
    out.textures_delta.clear();
    let texts = out
        .shapes
        .iter()
        .filter_map(|c| match &c.shape {
            egui::Shape::Text(t) => Some(t.galley.text().to_string()),
            _ => None,
        })
        .collect();
    (out_rows, texts)
}

fn click(ctx: &egui::Context, app: &mut VectorcraftApp, at: egui::Pos2, rows: fn(&mut VectorcraftApp, &mut egui::Ui) -> Rows) {
    frame(ctx, app, vec![egui::Event::PointerMoved(at), button(at, true)], rows);
    frame(ctx, app, vec![button(at, false)], rows);
}

fn para_rows(app: &mut VectorcraftApp, ui: &mut egui::Ui) -> Rows {
    let para = crate::panels::character::text_style(app).unwrap().1;
    let rtl = crate::varos::kashida_ui::targeted(app).first().is_some_and(|p| p.rtl);
    paragraph_rows(app, ui, &para, rtl)
}

fn char_rows(app: &mut VectorcraftApp, ui: &mut egui::Ui) -> Rows {
    let s = crate::panels::character::text_style(app).unwrap().0;
    character_rows(app, ui, &s)
}

/// The fork turns the Middle Eastern options on once, fresh or upgraded; the user's later choice
/// stands.
#[test]
fn middle_eastern_options_are_on_once_then_the_users_choice_stands() {
    let mut app = VectorcraftApp::new(Session::new(), Default::default());
    assert!(!app.ui.varos_middle_eastern);
    // An upgraded install: saved preferences with the options off.
    let mut saved = app.session.prefs.clone();
    saved.show_indic_options = false;
    app.ui.engine_prefs = saved.to_json();
    crate::prefs_dialog::restore(&mut app);
    assert!(app.session.prefs.show_indic_options && app.ui.varos_middle_eastern, "on once");
    // The user turns them off: that stays.
    app.session.prefs.show_indic_options = false;
    crate::prefs_dialog::snapshot(&mut app);
    crate::prefs_dialog::restore(&mut app);
    assert!(!app.session.prefs.show_indic_options, "the user's choice stands");
    // A fresh install (no saved preferences).
    let mut fresh = VectorcraftApp::new(Session::new(), Default::default());
    let mut p = vectorcraft_engine::Prefs::default();
    default_on(&mut fresh, &mut p);
    assert!(p.show_indic_options);
}

/// Direction: three icon buttons; Auto lit rings the side it resolved to; RTL and LTR set it; one
/// undo each.
#[test]
fn the_direction_buttons_set_auto_rtl_and_ltr() {
    let (mut app, id) = app_with("بسم الله الرحمن الرحيم");
    let ctx = egui::Context::default();
    crate::theme::install_fonts(&ctx);
    egui_extras::install_image_loaders(&ctx);
    let (rows, texts) = frame(&ctx, &mut app, vec![], para_rows);
    assert_eq!(rows.controls.len(), 3);
    assert!(texts.is_empty(), "icons only: {texts:?}");
    let (auto, rtl, ltr) = (rows.get("dir-auto"), rows.get("dir-rtl"), rows.get("dir-ltr"));
    assert!(auto.lit && !rtl.lit && !ltr.lit, "{rows:?}");
    assert!(rtl.ghost && !ltr.ghost, "Auto resolved to right to left: {rows:?}");
    assert!(auto.tip.contains("now right to left"), "{}", auto.tip);
    click(&ctx, &mut app, ltr.rect.center(), para_rows);
    assert_eq!(text(&app, id).para.direction, Some(ParaDirection::LeftToRight));
    let (rows, _) = frame(&ctx, &mut app, vec![], para_rows);
    assert!(rows.get("dir-ltr").lit && !rows.get("dir-auto").lit && !rows.get("dir-rtl").ghost, "{rows:?}");
    click(&ctx, &mut app, rows.get("dir-rtl").rect.center(), para_rows);
    assert_eq!(text(&app, id).para.direction, Some(ParaDirection::RightToLeft));
    click(&ctx, &mut app, rows.get("dir-auto").rect.center(), para_rows);
    assert_eq!(text(&app, id).para.direction, None);
    app.run("edit.undo", json!({})).unwrap();
    assert_eq!(text(&app, id).para.direction, Some(ParaDirection::RightToLeft));
    // Latin type: Auto resolves left to right.
    let (mut latin, _) = app_with("Typography");
    let (rows, _) = frame(&ctx, &mut latin, vec![], para_rows);
    assert!(rows.get("dir-auto").lit && rows.get("dir-ltr").ghost && !rows.get("dir-rtl").ghost, "{rows:?}");
    // Without the Middle Eastern options, no row.
    latin.session.prefs.show_indic_options = false;
    assert!(frame(&ctx, &mut latin, vec![], para_rows).0.controls.is_empty());
}

fn app_with_two(first: &str, second: &str) -> (VectorcraftApp, [u64; 2]) {
    let mut app = VectorcraftApp::new(Session::new(), Default::default());
    app.session.prefs.show_indic_options = true;
    app.run("file.new", json!({"width": 800, "height": 600})).unwrap();
    let ids = [(first, 50), (second, 300)].map(|(text, y)| {
        app.run("text.create", json!({"x": 20, "y": y, "text": text, "area": {"width": 300, "height": 200}, "justify": "justifyAll"})).unwrap()["id"]
            .as_u64()
            .unwrap()
    });
    app.session.execute("select.set", &json!({"ids": ids})).unwrap();
    (app, ids)
}

fn para_group(app: &mut VectorcraftApp, ui: &mut egui::Ui) -> Rows {
    let para = crate::panels::character::text_style(app).unwrap().1;
    let rtl = crate::varos::kashida_ui::targeted(app).first().is_some_and(|p| p.rtl);
    super::paragraph_group(app, ui, &para, rtl)
}

/// Variant C: one paragraph right to left, one left to right, their kashida different. Nothing
/// lights, the values they have are tinted, the strength reads "–"; a click sets both.
#[test]
fn a_mixed_selection_lights_nothing_and_tints_what_it_has() {
    let (mut app, [a, b]) = app_with_two("بسم الله الرحمن الرحيم", "الحمد لله رب العالمين");
    app.session.execute("select.set", &json!({"ids": [a]})).unwrap();
    app.run("text.setFormat", json!({"direction": "rightToLeft"})).unwrap();
    app.session.execute("select.set", &json!({"ids": [b]})).unwrap();
    app.run("text.setFormat", json!({"direction": "leftToRight", "kashida": "off", "kashidaStrength": 80})).unwrap();
    app.session.execute("select.set", &json!({"ids": [a, b]})).unwrap();
    let ctx = egui::Context::default();
    crate::theme::install_fonts(&ctx);
    egui_extras::install_image_loaders(&ctx);
    let (rows, texts) = frame(&ctx, &mut app, vec![], para_group);
    assert!(rows.controls.iter().filter(|c| c.key != "group").all(|c| !c.lit), "nothing lit: {rows:?}");
    for (key, tinted) in
        [("dir-auto", false), ("dir-rtl", true), ("dir-ltr", true), ("kashida-off", true), ("kashida-auto", true), ("kashida-force", false)]
    {
        assert_eq!(rows.get(key).tinted, tinted, "{key}: {rows:?}");
    }
    assert!(rows.controls.iter().all(|c| !c.ghost));
    assert!(texts.iter().any(|t| t == "–"), "{texts:?}");
    click(&ctx, &mut app, rows.get("dir-rtl").rect.center(), para_group);
    assert_eq!([a, b].map(|id| text(&app, id).para.direction), [Some(ParaDirection::RightToLeft); 2]);
    app.run("edit.undo", json!({})).unwrap();
    assert_eq!(text(&app, b).para.direction, Some(ParaDirection::LeftToRight), "one undo step");
    // A mixed digit setting reads "–" too.
    app.session.execute("select.set", &json!({"ids": [b]})).unwrap();
    app.run("text.setFormat", json!({"digits": "hindi"})).unwrap();
    app.session.execute("select.set", &json!({"ids": [a, b]})).unwrap();
    let (rows, texts) = frame(&ctx, &mut app, vec![], char_rows);
    assert!(!rows.get("digits").lit && texts.iter().any(|t| t == "–"), "{texts:?}");
}

/// Harakat: a preset, Hide and As letters act on the harakat only, the text untouched; the state
/// shows (the preset's ring, the lit toggle).
#[test]
fn the_harakat_row_colours_hides_and_restores_the_harakat() {
    let vocalized = "بِسْمِ اللَّهِ";
    let (mut app, id) = app_with(vocalized);
    let letters = text(&app, id).runs[0].style.fill.clone();
    let ctx = egui::Context::default();
    crate::theme::install_fonts(&ctx);
    egui_extras::install_image_loaders(&ctx);
    let (rows, _) = frame(&ctx, &mut app, vec![], char_rows);
    for key in ["digits", "hide", "as-letters", "harakat", "well", "preset-red", "preset-green", "preset-gold", "preset-black"] {
        rows.get(key);
    }
    assert!(rows.get("as-letters").lit && !rows.get("hide").lit, "they have their letters' colour: {rows:?}");
    assert!(rows.controls.iter().filter(|c| c.key.starts_with("preset-")).all(|c| !c.lit));
    let harakat_fills = |app: &VectorcraftApp| -> Vec<Paint> {
        text(app, id)
            .runs
            .iter()
            .flat_map(|r| r.text.chars().filter(|c| vectorcraft_text::harakat::is_harakat(*c)).map(move |_| r.style.fill.clone()))
            .collect()
    };
    click(&ctx, &mut app, rows.get("preset-red").rect.center(), char_rows);
    let red = harakat_fills(&app);
    assert_eq!(red.len(), 6);
    assert!(red.iter().all(|f| *f != letters && matches!(f, Paint::Solid { .. })), "{red:?}");
    assert_eq!(text(&app, id).plain_text(), vocalized);
    let (rows, _) = frame(&ctx, &mut app, vec![], char_rows);
    assert!(rows.get("preset-red").lit && !rows.get("preset-green").lit && !rows.get("as-letters").lit, "{rows:?}");
    click(&ctx, &mut app, rows.get("hide").rect.center(), char_rows);
    assert!(harakat_fills(&app).iter().all(|f| *f == Paint::None), "hidden");
    let (rows, _) = frame(&ctx, &mut app, vec![], char_rows);
    assert!(rows.get("hide").lit && !rows.get("preset-red").lit, "{rows:?}");
    // Shown again: the colour picked last.
    click(&ctx, &mut app, rows.get("hide").rect.center(), char_rows);
    assert_eq!(harakat_fills(&app), red, "back to red");
    click(&ctx, &mut app, rows.get("as-letters").rect.center(), char_rows);
    assert!(harakat_fills(&app).iter().all(|f| *f == letters), "as letters");
    let (rows, _) = frame(&ctx, &mut app, vec![], char_rows);
    assert!(rows.get("as-letters").lit, "{rows:?}");
    app.run("edit.undo", json!({})).unwrap();
    assert_eq!(harakat_fills(&app), red, "one undo step each");
}

/// The colour well opens a popover: a click on its colour field gives the harakat that colour
/// (any colour, not only the presets), and a typed hex the exact one, each one undo step, through
/// `text.styleHarakat`.
#[test]
fn the_harakat_colour_well_applies_any_colour() {
    let (mut app, id) = app_with("بِسْمِ اللَّهِ");
    let letters = text(&app, id).runs[0].style.fill.clone();
    let harakat_hexes = |app: &VectorcraftApp| -> Vec<String> {
        text(app, id)
            .runs
            .iter()
            .flat_map(|r| r.text.chars().filter(|c| vectorcraft_text::harakat::is_harakat(*c)).map(move |_| r.style.fill.clone()))
            .map(|f| match f {
                Paint::Solid { color, .. } => crate::panels::color::hex_digits(&color),
                other => format!("{other:?}"),
            })
            .collect()
    };
    let ctx = egui::Context::default();
    crate::theme::install_fonts(&ctx);
    egui_extras::install_image_loaders(&ctx);
    let steps = |app: &VectorcraftApp| app.session.active().unwrap().history.undo.len();
    let (rows, _) = frame(&ctx, &mut app, vec![], char_rows);
    click(&ctx, &mut app, rows.get("well").rect.center(), char_rows);
    let (rows, _) = frame(&ctx, &mut app, vec![], char_rows);
    let field = rows.get("harakat-field").rect;
    assert!(field.width() > 100.0, "the popover is open: {rows:?}");
    // Black has no hue: the field shows red hues (0°); 80 % saturation, 70 % brightness.
    let before = steps(&app);
    let at = egui::pos2(field.left() + field.width() * 0.8, field.top() + field.height() * 0.3);
    click(&ctx, &mut app, at, char_rows);
    let picked = harakat_hexes(&app);
    assert_eq!(picked.len(), 6);
    assert!(picked.windows(2).all(|w| w[0] == w[1]), "{picked:?}");
    let presets: Vec<String> = super::HARAKAT_COLOURS.iter().map(|(_, [r, g, b])| format!("{r:02X}{g:02X}{b:02X}")).collect();
    assert!(!presets.contains(&picked[0]), "an arbitrary colour: {picked:?}");
    let want = vectorcraft_color::Color::from_hsb(0.0, 0.8, 0.7);
    let got = match &text(&app, id).runs.iter().find(|r| r.text.chars().any(vectorcraft_text::harakat::is_harakat)).unwrap().style.fill {
        Paint::Solid { color, .. } => color.to_rgb(),
        f => panic!("{f:?}"),
    };
    let w = want.to_rgb();
    assert!(got.iter().zip(w).all(|(a, b)| (a - b).abs() < 0.03), "{got:?} ≈ {w:?}");
    assert_eq!(steps(&app), before + 1, "one step");
    assert_eq!(text(&app, id).runs.iter().find(|r| !r.text.chars().any(vectorcraft_text::harakat::is_harakat)).unwrap().style.fill, letters);
    // A typed hex: exactly that colour.
    let (rows, _) = frame(&ctx, &mut app, vec![], char_rows);
    let hex = rows.get("harakat-hex").rect;
    let field = egui::pos2(hex.right() - 30.0, hex.center().y);
    click(&ctx, &mut app, field, char_rows);
    let all = egui::Event::Key { key: egui::Key::A, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::COMMAND };
    frame(&ctx, &mut app, vec![all, egui::Event::Text("3A7BD5".into())], char_rows);
    let enter = |pressed| egui::Event::Key { key: egui::Key::Enter, physical_key: None, pressed, repeat: false, modifiers: Default::default() };
    frame(&ctx, &mut app, vec![enter(true), enter(false)], char_rows);
    frame(&ctx, &mut app, vec![], char_rows);
    assert!(harakat_hexes(&app).iter().all(|h| h == "3A7BD5"), "{:?}", harakat_hexes(&app));
    assert_eq!(steps(&app), before + 2);
}

/// Digits: the button shows the digits themselves; the list sets the setting.
#[test]
fn the_digits_dropdown_shows_and_sets_the_digits() {
    let (mut app, id) = app_with("سنة 2026");
    let ctx = egui::Context::default();
    crate::theme::install_fonts(&ctx);
    egui_extras::install_image_loaders(&ctx);
    let (rows, texts) = frame(&ctx, &mut app, vec![], char_rows);
    assert!(texts.iter().any(|t| t == "Aa") && texts.iter().any(|t| t == "Default"), "{texts:?}");
    click(&ctx, &mut app, rows.get("digits").rect.center(), char_rows);
    let (rows, _) = frame(&ctx, &mut app, vec![], char_rows);
    click(&ctx, &mut app, rows.get("digits-hindi").rect.center(), char_rows);
    assert_eq!(text(&app, id).runs[0].style.digits, vectorcraft_doc::Digits::Hindi);
    let (rows, texts) = frame(&ctx, &mut app, vec![], char_rows);
    // The digits one by one (egui would reverse a run of them), left to right.
    let at = |d: &str| texts.iter().position(|t| t == d).unwrap_or_else(|| panic!("{d} in {texts:?}"));
    assert!(at("٤") < at("٥") && at("٥") < at("٦") && texts.iter().any(|t| t == "Hindi"), "{texts:?}");
    assert!(rows.get("digits").tip.ends_with("Hindi"), "{}", rows.get("digits").tip);
}

/// Every control of both groups has a tool tip, and hovering it shows it.
#[test]
fn every_control_has_a_tooltip() {
    let ctx = egui::Context::default();
    crate::theme::install_fonts(&ctx);
    egui_extras::install_image_loaders(&ctx);
    ctx.global_style_mut(|s| {
        s.interaction.tooltip_delay = 0.0;
        s.interaction.show_tooltips_only_when_still = false;
    });
    let away = egui::pos2(310.0, 390.0);
    for (text, group) in [("بِسْمِ اللَّهِ 2026", para_group as fn(&mut VectorcraftApp, &mut egui::Ui) -> Rows), ("بِسْمِ اللَّهِ 2026", char_rows)]
    {
        let (mut app, _) = app_with(text);
        let (rows, _) = frame(&ctx, &mut app, vec![], group);
        assert!(rows.controls.len() >= 8, "{rows:?}");
        for c in &rows.controls {
            assert!(!c.tip.is_empty(), "{c:?}");
            frame(&ctx, &mut app, vec![egui::Event::PointerMoved(away)], group);
            frame(&ctx, &mut app, vec![egui::Event::PointerMoved(c.rect.center())], group);
            frame(&ctx, &mut app, vec![], group);
            let (_, texts) = frame(&ctx, &mut app, vec![], group);
            assert!(texts.contains(&c.tip), "{}: no tooltip {:?} in {texts:?}", c.key, c.tip);
        }
    }
}

/// The Glyphs panel lists the Arabic blocks; a face's characters in them, and none in a Latin font.
#[test]
fn the_glyphs_panel_has_the_arabic_blocks() {
    let subsets = crate::panels::glyphs::SUBSETS;
    let arabic: Vec<(&str, u32, u32)> = subsets.iter().copied().filter(|s| s.0.starts_with("Arabic")).collect();
    assert_eq!(
        arabic.iter().map(|s| (s.1, s.2)).collect::<Vec<_>>(),
        [(0x600, 0x6FF), (0x750, 0x77F), (0x8A0, 0x8FF), (0xFB50, 0xFDFF), (0xFE70, 0xFEFC)]
    );
    let db = vectorcraft_text::FontDb::with_font_dirs(vec![]);
    db.add_font(include_bytes!("../../../text-arabic/fixtures/NotoSansArabic.ttf").to_vec());
    let noto = db.face("Noto Sans Arabic", "Regular").unwrap();
    let latin = db.face("Source Sans 3", "Regular").unwrap();
    let in_block =
        |face: &vectorcraft_text::FontFace, (lo, hi): (u32, u32)| face.chars().into_iter().filter(|(c, _)| (lo..=hi).contains(&(*c as u32))).count();
    assert!(in_block(&noto, (0x600, 0x6FF)) > 100);
    for s in &arabic {
        assert_eq!(in_block(&latin, (s.1, s.2)), 0, "{}", s.0);
    }
}

/// Opt-in visual evidence of both groups in the real panels, Dark theme, 260 pt wide: set
/// `VAROS_PARAGRAPH_CAPTURE` to a directory. Needs a graphics adapter.
#[test]
#[ignore = "manual screenshot evidence; requires a graphics adapter"]
fn capture_the_middle_eastern_groups() {
    use egui_kittest::{Harness, kittest::Queryable};
    let Some(dir) = std::env::var_os("VAROS_PARAGRAPH_CAPTURE").map(std::path::PathBuf::from) else { return };
    std::fs::create_dir_all(&dir).unwrap();
    let shoot = |mut app: VectorcraftApp, character: bool, open: Option<&str>, name: &str| {
        app.session.prefs.show_indic_options = true;
        let mut ready = false;
        let mut h =
            Harness::builder().with_size(egui::vec2(300.0, if character { 470.0 } else { 300.0 })).with_pixels_per_point(2.0).wgpu().build_ui_state(
                move |ui, app: &mut VectorcraftApp| {
                    if !std::mem::replace(&mut ready, true) {
                        egui_extras::install_image_loaders(ui.ctx());
                        crate::theme::install_fonts(ui.ctx());
                        crate::theme::apply(ui.ctx(), crate::theme::Brightness::Dark);
                        return;
                    }
                    let t = crate::theme::Tokens::get(ui.ctx());
                    ui.painter().rect_filled(ui.max_rect(), 0, t.app_bar);
                    egui::Frame::NONE.fill(t.panel).corner_radius(8).inner_margin(egui::Margin::same(8)).show(ui, |ui| {
                        ui.set_width(244.0);
                        if character { crate::panels::character::show(app, ui) } else { crate::panels::paragraph::show(app, ui) }
                    });
                },
                app,
            );
        h.run_steps(4);
        if let Some(label) = open {
            h.get_by_label(label).click();
            h.run_steps(4);
        }
        for _ in 0..3 {
            std::thread::sleep(std::time::Duration::from_millis(80));
            h.run_steps(2);
        }
        h.render().expect("headless graphics adapter").save(dir.join(format!("{name}.png"))).unwrap();
    };
    // Paragraph, variant A: Arabic on Auto (resolved right to left), Kashida Auto at 50.
    let (app, _) = app_with("بسم الله الرحمن الرحيم الحمد لله رب العالمين");
    shoot(app, false, None, "paragraph-a");
    // Variant C: a right-to-left and a left-to-right paragraph, kashida different.
    let (mut app, [a, b]) = app_with_two("بسم الله الرحمن الرحيم", "الحمد لله رب العالمين");
    app.session.execute("select.set", &json!({"ids": [a]})).unwrap();
    app.run("text.setFormat", json!({"direction": "rightToLeft"})).unwrap();
    app.session.execute("select.set", &json!({"ids": [b]})).unwrap();
    app.run("text.setFormat", json!({"direction": "leftToRight", "kashida": "off", "kashidaStrength": 80})).unwrap();
    app.session.execute("select.set", &json!({"ids": [a, b]})).unwrap();
    shoot(app, false, None, "paragraph-c");
    // Kashida Off: Strength greys out.
    let (mut app, _) = app_with("بسم الله الرحمن الرحيم");
    app.run("text.setFormat", json!({"kashida": "off"})).unwrap();
    shoot(app, false, None, "paragraph-off");
    // Character: Hindi digits, red harakat; the colour well open; the Digits list open; hidden.
    let vocalized = || {
        let (mut app, _) = app_with("بِسْمِ اللَّهِ ٢٠٢٦");
        app.run("text.setFormat", json!({"digits": "hindi"})).unwrap();
        app.run("text.styleHarakat", json!({"fill": "#C0392B"})).unwrap();
        app
    };
    shoot(vocalized(), true, None, "character");
    shoot(vocalized(), true, Some("Harakat colour: click for any colour"), "character-well");
    shoot(vocalized(), true, Some("Digits"), "character-digits");
    let mut hidden = vocalized();
    hidden.run("text.styleHarakat", json!({"fill": "none"})).unwrap();
    shoot(hidden, true, None, "character-hidden");
}
