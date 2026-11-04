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

#[test]
fn native_checkbox_uses_the_shared_checkbox_role() {
    let ctx = themed(theme::Brightness::Dark);
    let mut checked = true;
    let mut bounds = Rect::NOTHING;
    let mut out = frame(&ctx, 0.0, vec![], |ui| bounds = ui.checkbox(&mut checked, "Enabled").rect);
    let mut rects = vec![];
    for shape in &out.shapes {
        rectangles(&shape.shape, &mut rects);
    }
    let painted = rects
        .iter()
        .filter(|r| r.fill != Color32::TRANSPARENT && bounds.expand(1.0).contains_rect(r.rect))
        .max_by(|a, b| a.rect.area().total_cmp(&b.rect.area()))
        .expect("native checkbox frame");
    assert_eq!(painted.corner_radius, egui::CornerRadius::same(FOUNDATION.checkbox_radius));
    out.textures_delta.clear();
}

fn control_fill(output: &egui::FullOutput, bounds: Rect) -> Option<Color32> {
    let mut rects = vec![];
    for shape in &output.shapes {
        rectangles(&shape.shape, &mut rects);
    }
    rects
        .into_iter()
        .filter(|rect| rect.fill != Color32::TRANSPARENT && bounds.expand(1.0).contains_rect(rect.rect))
        .max_by(|a, b| a.rect.area().total_cmp(&b.rect.area()))
        .map(|rect| rect.fill)
}

#[test]
fn native_and_shared_secondary_buttons_share_color_roles_without_recoloring_fields() {
    for brightness in [theme::Brightness::Dark, theme::Brightness::Light] {
        let ctx = themed(brightness);
        let t = theme::Tokens::get(&ctx);
        let native = Cell::new(Rect::NOTHING);
        let shared = Cell::new(Rect::NOTHING);
        let field = Cell::new(Rect::NOTHING);
        let mut text = "Text عربي".to_owned();
        let mut draw = |ui: &mut Ui| {
            native.set(ui.button("Native secondary").rect);
            shared.set(widgets::secondary_button(ui, "Shared secondary").rect);
            field.set(ui.add(egui::TextEdit::singleline(&mut text)).rect);
        };
        for time in [0.0, 0.1] {
            let mut out = frame(&ctx, time, vec![], &mut draw);
            out.textures_delta.clear();
            if time > 0.0 {
                assert_eq!(control_fill(&out, native.get()), Some(t.button), "{brightness:?}: native button");
                assert_eq!(control_fill(&out, shared.get()), Some(t.button), "{brightness:?}: shared button");
                assert_eq!(control_fill(&out, field.get()), Some(t.input), "{brightness:?}: text field");
            }
        }
        let mut time = 0.2;
        for bounds in [native.get(), shared.get()] {
            for _ in 0..2 {
                let mut out = frame(&ctx, time, vec![Event::PointerMoved(bounds.center())], &mut draw);
                out.textures_delta.clear();
                time += 0.1;
            }
            let mut out = frame(&ctx, time, vec![Event::PointerMoved(bounds.center())], &mut draw);
            out.textures_delta.clear();
            time += 0.1;
            assert_eq!(control_fill(&out, bounds), Some(t.hover), "{brightness:?}: hovered secondary button");
            assert_eq!(control_fill(&out, field.get()), Some(t.input), "button hover must not recolor fields");
        }
    }
}

#[test]
fn button_color_roles_preserve_frameless_selected_explicit_fill_and_combo_states() {
    for brightness in [theme::Brightness::Dark, theme::Brightness::Light] {
        let ctx = themed(brightness);
        let t = theme::Tokens::get(&ctx);
        let plain = Cell::new(Rect::NOTHING);
        let selected = Cell::new(Rect::NOTHING);
        let explicit = Cell::new(Rect::NOTHING);
        let combo = Cell::new(Rect::NOTHING);
        let drag_value = Cell::new(Rect::NOTHING);
        let mut numeric_value = 42.0;
        let combo_id = Cell::new(Id::NULL);
        let explicit_fill = Color32::from_rgb(71, 101, 131);
        let mut draw = |ui: &mut Ui| {
            plain.set(ui.add(egui::Button::new("Frameless").frame(false)).rect);
            selected.set(ui.add(egui::Button::new("Selected").selected(true)).rect);
            explicit.set(ui.add(egui::Button::new("Explicit fill").fill(explicit_fill)).rect);
            drag_value.set(ui.add(egui::DragValue::new(&mut numeric_value)).rect);
            let response = egui::ComboBox::from_id_salt("shared-colors-combo")
                .selected_text("Normal")
                .show_ui(ui, |ui| {
                    ui.label("Option");
                })
                .response;
            combo.set(response.rect);
            combo_id.set(response.id);
        };
        for time in [0.0, 0.1] {
            let mut out = frame(&ctx, time, vec![], &mut draw);
            out.textures_delta.clear();
            if time > 0.0 {
                assert_eq!(control_fill(&out, plain.get()), None, "frameless buttons must remain unfilled");
                assert_eq!(control_fill(&out, selected.get()), Some(t.accent), "selected button keeps selection fill");
                assert_eq!(control_fill(&out, explicit.get()), Some(explicit_fill), "explicit button fill wins");
                assert_eq!(control_fill(&out, combo.get()), Some(t.input), "closed combo keeps the field role");
                assert_eq!(control_fill(&out, drag_value.get()), Some(t.input), "DragValue uses ButtonStyle internally but remains a numeric field");
            }
        }
        let at = combo.get().center();
        for (time, pressed) in [(0.2, true), (0.3, false)] {
            let mut out = frame(&ctx, time, vec![Event::PointerMoved(at), pointer(at, pressed)], &mut draw);
            out.textures_delta.clear();
        }
        assert!(egui::ComboBox::is_open(&ctx, combo_id.get()), "the real combo opens");
        let mut out = frame(&ctx, 0.4, vec![Event::PointerMoved(Pos2::new(600.0, 580.0))], &mut draw);
        out.textures_delta.clear();
        assert_eq!(control_fill(&out, combo.get()), Some(t.tool_active), "open combo retains its open-field role");
        assert_eq!(control_fill(&out, drag_value.get()), Some(t.input));
        assert_eq!(control_fill(&out, plain.get()), None);
        assert_eq!(control_fill(&out, selected.get()), Some(t.accent));
        assert_eq!(control_fill(&out, explicit.get()), Some(explicit_fill));
    }
}

#[test]
fn native_text_field_border_roles_follow_hover_and_focus_without_overriding_explicit_frames() {
    for brightness in [theme::Brightness::Dark, theme::Brightness::Light] {
        let ctx = themed(brightness);
        let t = theme::Tokens::get(&ctx);
        let field_id = Id::unique("shared-field-border");
        let native = Cell::new(Rect::NOTHING);
        let unframed = Cell::new(Rect::NOTHING);
        let readonly_rect = Cell::new(Rect::NOTHING);
        let explicit = Cell::new(Rect::NOTHING);
        let mut text = "Text عربي".to_owned();
        let mut plain_text = "Unframed".to_owned();
        let mut readonly = "Read-only";
        let mut explicit_text = "Explicit frame".to_owned();
        let explicit_fill = Color32::from_rgb(71, 101, 131);
        let explicit_border = Color32::from_rgb(181, 101, 51);
        let mut draw = |ui: &mut Ui| {
            native.set(ui.add(egui::TextEdit::singleline(&mut text).id(field_id)).rect);
            unframed.set(ui.add(egui::TextEdit::singleline(&mut plain_text).frame(egui::Frame::NONE)).rect);
            readonly_rect.set(ui.add(egui::TextEdit::singleline(&mut readonly)).rect);
            explicit.set(
                ui.add(
                    egui::TextEdit::singleline(&mut explicit_text)
                        .frame(egui::Frame::NONE.fill(explicit_fill).stroke(egui::Stroke::new(2.0, explicit_border))),
                )
                .rect,
            );
        };
        let painted_frame = |out: &egui::FullOutput, bounds: Rect| {
            let mut rects = vec![];
            for shape in &out.shapes {
                rectangles(&shape.shape, &mut rects);
            }
            rects
                .into_iter()
                .filter(|r| r.fill != Color32::TRANSPARENT && bounds.expand(1.0).contains_rect(r.rect))
                .max_by(|a, b| a.rect.area().total_cmp(&b.rect.area()))
                .cloned()
                .expect("actual framed text field")
        };
        for time in [0.0, 0.1] {
            let mut out = frame(&ctx, time, vec![], &mut draw);
            out.textures_delta.clear();
            if time > 0.0 {
                let field = painted_frame(&out, native.get());
                assert_eq!(field.fill, t.input);
                assert_eq!(field.stroke.color, t.input_border);
                assert_eq!(control_fill(&out, unframed.get()), None, "explicit Frame::NONE stays transparent");
                assert_eq!(control_fill(&out, readonly_rect.get()), None, "read-only text retains transparent background");
                let custom = painted_frame(&out, explicit.get());
                assert_eq!((custom.fill, custom.stroke.color, custom.stroke.width), (explicit_fill, explicit_border, 2.0));
            }
        }
        let at = native.get().center();
        for time in [0.2, 0.3] {
            let mut out = frame(&ctx, time, vec![Event::PointerMoved(at)], &mut draw);
            out.textures_delta.clear();
            let field = painted_frame(&out, native.get());
            assert_eq!((field.fill, field.stroke.color), (t.input, t.input_border), "hover keeps the field role");
        }
        ctx.memory_mut(|memory| memory.request_focus(field_id));
        for time in [0.4, 0.5] {
            let mut out = frame(&ctx, time, vec![Event::PointerMoved(Pos2::new(600.0, 580.0))], &mut draw);
            out.textures_delta.clear();
            if time > 0.4 {
                let field = painted_frame(&out, native.get());
                assert_eq!((field.fill, field.stroke.color), (t.input, t.accent), "focused fields use the shared accent border");
                assert_eq!(control_fill(&out, unframed.get()), None);
                assert_eq!(control_fill(&out, readonly_rect.get()), None);
            }
        }
    }
}
