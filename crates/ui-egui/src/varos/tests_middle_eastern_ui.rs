//! Varos: the Middle Eastern rows ([`super`]) and the Glyphs panel's Arabic subsets, driven
//! headless.

use serde_json::json;
use vectorcraft_color::Paint;
use vectorcraft_doc::{NodeKind, ParaDirection, TextObject};
use vectorcraft_engine::Session;

use super::{Rows, character_rows, default_on, paragraph_rows};
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
    let rtl = crate::varos::kashida_ui::targeted(app).first().is_some_and(|(_, rtl)| *rtl);
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

/// Direction: Auto names what it resolved to; RTL and LTR set it; one undo each.
#[test]
fn the_direction_row_sets_auto_rtl_and_ltr() {
    let (mut app, id) = app_with("بسم الله الرحمن الرحيم");
    let ctx = egui::Context::default();
    crate::theme::install_fonts(&ctx);
    let (rows, texts) = frame(&ctx, &mut app, vec![], para_rows);
    assert_eq!(rows.direction.len(), 3);
    assert!(texts.iter().any(|t| t == "Auto (RTL)"), "{texts:?}");
    click(&ctx, &mut app, rows.direction[2].center(), para_rows);
    assert_eq!(text(&app, id).para.direction, Some(ParaDirection::LeftToRight));
    click(&ctx, &mut app, rows.direction[1].center(), para_rows);
    assert_eq!(text(&app, id).para.direction, Some(ParaDirection::RightToLeft));
    let (_, texts) = frame(&ctx, &mut app, vec![], para_rows);
    assert!(texts.iter().any(|t| t == "Auto") && !texts.iter().any(|t| t.starts_with("Auto (")), "{texts:?}");
    click(&ctx, &mut app, rows.direction[0].center(), para_rows);
    assert_eq!(text(&app, id).para.direction, None);
    app.run("edit.undo", json!({})).unwrap();
    assert_eq!(text(&app, id).para.direction, Some(ParaDirection::RightToLeft));
    // Latin type: Auto resolves left to right.
    let (mut latin, _) = app_with("Typography");
    let (_, texts) = frame(&ctx, &mut latin, vec![], para_rows);
    assert!(texts.iter().any(|t| t == "Auto (LTR)"), "{texts:?}");
    // Without the Middle Eastern options, no row.
    latin.session.prefs.show_indic_options = false;
    assert!(frame(&ctx, &mut latin, vec![], para_rows).0.direction.is_empty());
}

/// Harakat: a colour, Hide and As Letters act on the harakat only, the text untouched.
#[test]
fn the_harakat_row_colours_hides_and_restores_the_harakat() {
    let vocalized = "بِسْمِ اللَّهِ";
    let (mut app, id) = app_with(vocalized);
    let letters = text(&app, id).runs[0].style.fill.clone();
    let ctx = egui::Context::default();
    crate::theme::install_fonts(&ctx);
    let (rows, texts) = frame(&ctx, &mut app, vec![], char_rows);
    assert!(texts.iter().any(|t| t == "Digits") && texts.iter().any(|t| t == "Harakat"), "{texts:?}");
    assert_eq!(rows.harakat.len(), 6);
    let harakat_fills = |app: &VectorcraftApp| -> Vec<Paint> {
        text(app, id)
            .runs
            .iter()
            .flat_map(|r| r.text.chars().filter(|c| vectorcraft_text::harakat::is_harakat(*c)).map(move |_| r.style.fill.clone()))
            .collect()
    };
    click(&ctx, &mut app, rows.harakat[0].center(), char_rows);
    let red = harakat_fills(&app);
    assert_eq!(red.len(), 6);
    assert!(red.iter().all(|f| *f != letters && matches!(f, Paint::Solid { .. })), "{red:?}");
    assert_eq!(text(&app, id).plain_text(), vocalized);
    click(&ctx, &mut app, rows.harakat[4].center(), char_rows);
    assert!(harakat_fills(&app).iter().all(|f| *f == Paint::None), "hidden");
    click(&ctx, &mut app, rows.harakat[5].center(), char_rows);
    assert!(harakat_fills(&app).iter().all(|f| *f == letters), "as letters");
    app.run("edit.undo", json!({})).unwrap();
    assert!(harakat_fills(&app).iter().all(|f| *f == Paint::None), "one undo step each");
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
