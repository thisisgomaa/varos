use crate::VectorcraftApp;
use egui::{Key, vec2};
use egui_kittest::{Harness, kittest::Queryable};
use serde_json::json;
use vectorcraft_engine::Session;

fn app() -> VectorcraftApp {
    let mut app = VectorcraftApp::new(Session::new(), Default::default());
    app.run("file.new", json!({"width":300,"height":200,"artboards":1})).unwrap();
    app.run("page.add", json!({"name":"Details"})).unwrap();
    app
}
fn harness() -> Harness<'static, VectorcraftApp> {
    let mut fonts = false;
    let mut h = Harness::builder().with_step_dt(1.0 / 60.0).with_size(vec2(340.0, 420.0)).build_ui_state(
        move |ui, app: &mut VectorcraftApp| {
            if !std::mem::replace(&mut fonts, true) {
                crate::theme::install_fonts(ui.ctx());
                egui_extras::install_image_loaders(ui.ctx());
                return;
            }
            crate::theme::apply(ui.ctx(), crate::theme::Brightness::Dark);
            super::pages::show(app, ui);
        },
        app(),
    );
    h.run_steps(4);
    h
}
#[test]
fn pages_rows_switch_and_window_panel_registration() {
    let mut h = harness();
    assert!(crate::icons::exists("file"));
    assert_eq!(crate::state::rail_panel("pages"), Some(("pages", "Pages", "file")));
    h.state_mut().run("window.panel", json!({"panel":"pages"})).unwrap();
    let row = h.get_by_label("Details 1").rect();
    assert_eq!(row.height(), 28.0);
    h.get_by_label("Details 1").click();
    h.run_steps(3);
    assert_eq!(h.state().session.project().unwrap().active_page(), 1);
    assert!(super::pages::status(h.state()).unwrap().starts_with("Page 2 of 2"));
}
#[test]
fn pages_context_menu_rename_duplicate_delete_and_reorder() {
    let mut h = harness();
    h.get_by_label("Details 1").click_secondary();
    h.run_steps(3);
    h.get_by_label("Rename").click();
    h.run_steps(3);
    let field = h.get_by_role(egui::accesskit::Role::TextInput);
    field.set_text_value_accesskit("Specifications");
    h.run_steps(2);
    h.key_press(Key::Enter);
    h.run_steps(3);
    assert_eq!(h.state().session.project().unwrap().pages()[1].name, "Specifications");
    h.get_by_label("Specifications 1").click_secondary();
    h.run_steps(3);
    h.get_by_label("Move up").click();
    h.run_steps(3);
    assert_eq!(h.state().session.project().unwrap().pages()[0].name, "Specifications");
    h.get_by_label("Specifications 1").click_secondary();
    h.run_steps(3);
    h.get_by_label("Duplicate").click();
    h.run_steps(3);
    assert_eq!(h.state().session.project().unwrap().pages().len(), 3);
    h.get_by_label("Specifications copy 1").click_secondary();
    h.run_steps(3);
    h.get_by_label("Delete").click();
    h.run_steps(3);
    assert_eq!(h.state().session.project().unwrap().pages().len(), 2);
}
#[test]
fn pages_drag_reorders_and_active_row_has_azure_fill() {
    let mut h = harness();
    let from = h.get_by_label("Details 1").rect().center();
    let to = h.get_by_label("Page 1 1").rect().center();
    h.drag_at(from);
    h.run_steps(1);
    h.event(egui::Event::PointerMoved(from + vec2(0.0, -10.0)));
    h.run_steps(1);
    h.event(egui::Event::PointerMoved(to));
    h.run_steps(1);
    h.event(egui::Event::PointerButton { pos: to, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() });
    h.run_steps(3);
    assert_eq!(h.state().session.project().unwrap().pages()[0].name, "Details");
    let ctx = egui::Context::default();
    crate::theme::install_fonts(&ctx);
    crate::theme::apply(&ctx, crate::theme::Brightness::Dark);
    let tint = crate::theme::Tokens::get(&ctx).accent.gamma_multiply(0.22);
    for _ in 0..3 {
        let mut out = ctx.run_ui(Default::default(), |ui| super::pages::show(h.state_mut(), ui));
        fn has(s: &egui::epaint::Shape, t: egui::Color32) -> bool {
            match s {
                egui::epaint::Shape::Rect(r) => r.fill == t,
                egui::epaint::Shape::Vec(v) => v.iter().any(|s| has(s, t)),
                _ => false,
            }
        }
        if out.shapes.iter().any(|s| has(&s.shape, tint)) {
            out.textures_delta.clear();
            return;
        }
        out.textures_delta.clear();
    }
    panic!("active row must paint the azure selection tint");
}
#[test]
#[ignore = "software visual capture; set VAROS_PAGES_CAPTURE"]
fn capture_pages_panel() {
    let Some(path) = std::env::var_os("VAROS_PAGES_CAPTURE") else { return };
    let mut app = app();
    app.run("page.rename", json!({"page":0,"name":"Overview"})).unwrap();
    app.run("page.add", json!({"name":"Explorations"})).unwrap();
    app.run("page.switch", json!({"page":1})).unwrap();
    let ctx = egui::Context::default();
    crate::theme::install_fonts(&ctx);
    crate::theme::apply(&ctx, crate::theme::Brightness::Dark);
    egui_extras::install_image_loaders(&ctx);
    let mut textures = std::collections::HashMap::<egui::TextureId, egui::ColorImage>::new();
    let (width, height) = (360, 320);
    let mut primitives = Vec::new();
    for frame in 0..8 {
        let raw = egui::RawInput {
            time: Some(frame as f64 * 0.1),
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(width as f32, height as f32))),
            ..Default::default()
        };
        let mut out = ctx.run_ui(raw, |ui| {
            egui::Frame::NONE.fill(crate::theme::Tokens::get(ui.ctx()).panel).inner_margin(16.0).show(ui, |ui| {
                ui.set_min_size(vec2(width as f32 - 32.0, height as f32 - 32.0));
                super::pages::show(&mut app, ui);
            });
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
        primitives = ctx.tessellate(out.shapes, out.pixels_per_point);
    }
    let pixels = super::home::capture::raster(&primitives, &textures, width, height);
    image::save_buffer(path, &pixels, width, height, image::ColorType::Rgba8).unwrap();
}

#[test]
fn pages_double_click_renames_inline_and_plus_uses_the_command() {
    let mut h = harness();
    h.get_by_label("Add page").click();
    h.run_steps(3);
    assert_eq!(h.state().session.project().unwrap().pages().len(), 3);
    h.run_steps(30); // Separate the Add click from the row double-click.
    h.get_by_label("Details 1").click();
    h.run_steps(1);
    h.get_by_label("Details 1").click();
    h.run_steps(2);
    h.get_by_role(egui::accesskit::Role::TextInput).set_text_value_accesskit("Notes");
    h.run_steps(1);
    h.key_press(Key::Enter);
    h.run_steps(3);
    assert_eq!(h.state().session.project().unwrap().pages()[1].name, "Notes");
}
