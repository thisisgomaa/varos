//! Real shared controls: presentation contracts and preserved keyboard/disabled behavior.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use super::theme_components::FOUNDATION;
use crate::{theme, widgets};
use egui::{Color32, Event, Id, Key, Modifiers, Pos2, Rect, Shape, Ui, vec2};
use std::cell::Cell;

fn themed(brightness: theme::Brightness) -> egui::Context {
    crate::i18n::set_current(crate::i18n::Lang::EN);
    let ctx = egui::Context::default();
    theme::install_fonts(&ctx);
    theme::apply(&ctx, brightness);
    egui_extras::install_image_loaders(&ctx);
    ctx
}
fn frame(ctx: &egui::Context, time: f64, events: Vec<Event>, mut draw: impl FnMut(&mut Ui)) -> egui::FullOutput {
    ctx.run_ui(
        egui::RawInput {
            time: Some(time),
            events,
            focused: true,
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(640.0, 620.0))),
            max_texture_side: Some(8192),
            ..Default::default()
        },
        |ui| draw(ui),
    )
}
fn key(key: Key) -> Event {
    Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }
}
fn pointer(at: Pos2, pressed: bool) -> Event {
    Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE }
}
fn gallery(ui: &mut Ui, native_text: &mut String) -> Vec<(&'static str, Rect)> {
    ui.painter().rect_filled(ui.max_rect(), 0.0, theme::Tokens::get(ui.ctx()).panel);
    ui.heading("Shared controls — الأزرار والحقول");
    let controls = vec![
        ("native button", ui.button("Native button").rect),
        ("flat button", widgets::flat_button(ui, "Flat action", 140.0).rect),
        ("primary button", widgets::primary_button(ui, "Confirm").rect),
        ("secondary button", widgets::secondary_button(ui, "Cancel").rect),
        ("icon button", widgets::icon_button(ui, "plus", "Add", true, FOUNDATION.control_height).rect),
        ("disabled button", widgets::primary_button_enabled(ui, "Unavailable", false).rect),
        ("native field", ui.add(egui::TextEdit::singleline(native_text).hint_text("Native field")).rect),
        ("numeric field", ui.scope(|ui| widgets::plain_field_named(ui, "gallery-number", Some("Value"), 42.0, "%", 0, 120.0)).response.rect),
        ("text field", ui.scope(|ui| widgets::text_field_named(ui, "gallery-text", Some("Name"), Some("اسم عربي"), 160.0, 1)).response.rect),
    ];
    let _ = widgets::combo_named(ui, "gallery-combo", Some("Mode"), "Normal", 140.0, false, |_ui| None::<()>);
    controls
}
fn rectangles<'a>(shape: &'a Shape, out: &mut Vec<&'a egui::epaint::RectShape>) {
    match shape {
        Shape::Rect(rect) => out.push(rect),
        Shape::Vec(shapes) => shapes.iter().for_each(|shape| rectangles(shape, out)),
        _ => {}
    }
}

#[test]
fn actual_shared_buttons_and_fields_use_the_foundation_corners_in_dark_and_light() {
    for brightness in [theme::Brightness::Dark, theme::Brightness::Light] {
        let ctx = themed(brightness);
        let mut text = "Text عربي".to_owned();
        let mut controls = vec![];
        for time in [0.0, 0.1] {
            let mut out = frame(&ctx, time, vec![], |ui| controls = gallery(ui, &mut text));
            out.textures_delta.clear();
            if time > 0.0 {
                let mut rects = vec![];
                for shape in &out.shapes {
                    rectangles(&shape.shape, &mut rects);
                }
                for (name, bounds) in &controls {
                    let painted = rects
                        .iter()
                        .filter(|rect| rect.fill != Color32::TRANSPARENT && bounds.expand(1.0).contains_rect(rect.rect))
                        .max_by(|a, b| a.rect.area().total_cmp(&b.rect.area()))
                        .unwrap_or_else(|| panic!("{name}: no painted control box"));
                    assert_eq!(painted.corner_radius, egui::CornerRadius::same(FOUNDATION.control_radius), "{brightness:?}: {name}");
                }
            }
            out.textures_delta.clear();
        }
    }
}

#[test]
fn shared_numeric_field_enter_commits_and_escape_discards_the_next_edit() {
    let ctx = themed(theme::Brightness::Dark);
    let id = Cell::new(Id::NULL);
    let value = Cell::new(7.0);
    let committed = Cell::new(None);
    let mut draw = |ui: &mut Ui| {
        id.set(ui.scope_id().with("number"));
        committed.set(widgets::plain_field_named(ui, "number", Some("Value"), value.get(), "", 0, 120.0));
        if let Some(next) = committed.get() {
            value.set(next);
        }
    };
    let mut out = frame(&ctx, 0.0, vec![], &mut draw);
    out.textures_delta.clear();
    let mut time = 0.1;
    for (typed, finish, expected) in [("23", Key::Enter, 23.0), ("99", Key::Escape, 23.0)] {
        ctx.memory_mut(|memory| memory.request_focus(id.get()));
        let mut out = frame(&ctx, time, vec![], &mut draw);
        out.textures_delta.clear();
        time += 0.1;
        let mut state = egui::TextEdit::load_state(&ctx, id.get()).unwrap_or_default();
        state.cursor.set_char_range(Some(egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(20))));
        state.store(&ctx, id.get());
        let mut out = frame(&ctx, time, vec![Event::Text(typed.into())], &mut draw);
        out.textures_delta.clear();
        time += 0.1;
        assert_eq!(value.get(), if finish == Key::Enter { 7.0 } else { 23.0 }, "typing must not commit before Enter");
        let mut out = frame(&ctx, time, vec![key(finish)], &mut draw);
        out.textures_delta.clear();
        time += 0.1;
        assert_eq!(value.get(), expected);
        assert!(!ctx.memory(|memory| memory.has_focus(id.get())), "Enter/Escape leaves the field");
    }
}

#[test]
fn disabled_shared_button_cannot_activate_or_steal_field_focus() {
    let ctx = themed(theme::Brightness::Dark);
    let button = Cell::new(Rect::NOTHING);
    let clicked = Cell::new(false);
    let button_id = Cell::new(Id::NULL);
    let field_id = Id::unique("disabled-button-neighbor");
    let mut text = "Keep typing".to_owned();
    let mut draw = |ui: &mut Ui| {
        ui.add(egui::TextEdit::singleline(&mut text).id(field_id));
        let response = widgets::primary_button_enabled(ui, "Unavailable", false);
        button.set(response.rect);
        button_id.set(response.id);
        clicked.set(response.clicked());
    };
    let mut out = frame(&ctx, 0.0, vec![], &mut draw);
    out.textures_delta.clear();
    let at = button.get().center();
    ctx.memory_mut(|memory| memory.request_focus(field_id));
    for (time, pressed) in [(0.1, true), (0.2, false)] {
        let mut out = frame(&ctx, time, vec![Event::PointerMoved(at), pointer(at, pressed)], &mut draw);
        out.textures_delta.clear();
        assert!(!clicked.get());
    }
    // Clicking elsewhere may end the text field's focus; the disabled button itself must not own it.
    assert_ne!(ctx.memory(|memory| memory.focused()), Some(button_id.get()));
}

#[test]
#[ignore = "optional software gallery; set VAROS_CONTROLS_CAPTURE to an output directory"]
fn capture_control_foundation_gallery() {
    let Some(path) = std::env::var_os("VAROS_CONTROLS_CAPTURE") else { return };
    let path = std::path::PathBuf::from(path);
    std::fs::create_dir_all(&path).unwrap();
    for (name, brightness) in [("dark", theme::Brightness::Dark), ("light", theme::Brightness::Light)] {
        let ctx = themed(brightness);
        let mut textures = std::collections::HashMap::<egui::TextureId, egui::ColorImage>::new();
        let mut native_text = "Text عربي".to_owned();
        let mut shot = vec![];
        for step in 0..4 {
            let mut out = frame(&ctx, step as f64 * 0.1, vec![], |ui| {
                gallery(ui, &mut native_text);
            });
            for (id, deltas) in &out.textures_delta.set {
                for delta in deltas {
                    let egui::ImageData::Color(image) = &delta.image;
                    if let Some([x, y]) = delta.pos {
                        let target = textures.get_mut(id).unwrap();
                        for row in 0..image.size[1] {
                            for col in 0..image.size[0] {
                                target.pixels[(y + row) * target.size[0] + x + col] = image.pixels[row * image.size[0] + col];
                            }
                        }
                    } else {
                        textures.insert(*id, (**image).clone());
                    }
                }
            }
            out.textures_delta.clear();
            shot = ctx.tessellate(out.shapes, out.pixels_per_point);
        }
        let pixels = super::home::capture::raster(&shot, &textures, 640, 620);
        image::save_buffer(path.join(format!("controls-{name}.png")), &pixels, 640, 620, image::ColorType::Rgba8).unwrap();
    }
}
