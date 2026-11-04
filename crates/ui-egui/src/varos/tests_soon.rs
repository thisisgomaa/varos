//! Actual surface frames, accessibility, physical input and software evidence.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use super::*;
use crate::VectorcraftApp;
use egui::{Id, Pos2, Shape, vec2};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use serde_json::json;

#[derive(Clone, Copy, Debug)]
enum Surface {
    Ask,
    Images,
    History,
    Settings,
    Object,
    Plugins,
    Dieline,
    Window,
    Setup,
    Color,
    Home,
    Links,
    Share,
    Legacy,
}
const SURFACES: &[(Surface, &[&str])] = &[
    (
        Surface::Ask,
        &[
            "Chat with Codex / Claude Code",
            "Chat with your own API key",
            "Project memory",
            "Recipes & quick scripts",
            "Agents on your server",
            "Build an extension with AI",
        ],
    ),
    (
        Surface::Images,
        &[
            "Generate images",
            "Several versions",
            "Styles",
            "Find references",
            "Edit with a prompt",
            "Expand image",
            "Remove object",
            "Remove background",
            "Upscale",
            "Adjustments",
            "Auto merge",
        ],
    ),
    (Surface::History, &["History per AI", "Undo this run"]),
    (Surface::Settings, &["Image providers & keys"]),
    (Surface::Object, &["Dieline"]),
    (Surface::Plugins, &["Extension Library…"]),
    (Surface::Dieline, &["Make Dieline…"]),
    (Surface::Window, &["3D Box Preview", "Comments", "Preflight"]),
    (Surface::Setup, &["Print rules"]),
    (Surface::Color, &["Choose press condition"]),
    (Surface::Home, &["Shared with me", "Join with a code", "Box templates", "Brand guide template"]),
    (Surface::Links, &["Copy placed images into the project folder"]),
    (Surface::Share, &["Share"]),
];

fn app(surface: Surface) -> VectorcraftApp {
    let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
    app.run("file.new", json!({"width":300, "height":300})).unwrap();
    app.run("shape.rectangle", json!({"x":10, "y":10, "width":20, "height":20})).unwrap();
    match surface {
        Surface::Legacy => {
            app.ui.varos_shell.enabled = true;
            crate::varos::ai::initialize_fresh(&mut app.ui);
        }
        Surface::Share => {
            app.run("ui.panelReview", json!({"enabled":true})).unwrap();
            app.run("window.workspace", json!({"name":"Studio"})).unwrap();
            app.ui.varos_panel_review.as_mut().unwrap().initialize_ai();
        }
        Surface::Settings => {
            app.run("edit.preferences", json!({"category":"AI"})).unwrap();
        }
        Surface::Setup => {
            app.run("file.documentSetup", json!({})).unwrap();
        }
        Surface::Home => {
            app.run("app.home", json!({"section":"templates"})).unwrap();
        }
        _ => {}
    }
    app
}
fn setup(ctx: &egui::Context, surface: Surface) {
    crate::i18n::set_current(crate::i18n::Lang::EN);
    crate::theme::install_fonts(ctx);
    crate::theme::apply(ctx, crate::theme::Brightness::Dark);
    egui_extras::install_image_loaders(ctx);
    ctx.all_styles_mut(|s| {
        s.interaction.tooltip_delay = 0.0;
        s.interaction.tooltip_grace_time = 0.0;
        s.interaction.show_tooltips_only_when_still = false;
    });
    let tab = match surface {
        Surface::Images => 1usize,
        Surface::History => 2,
        _ => 0,
    };
    ctx.data_mut(|d| d.insert_temp(Id::unique("varos-ai-tab"), tab));
}
fn menu(ui: &mut Ui, app: &VectorcraftApp, title: &str, child: Option<&str>) {
    let tree = crate::menus::menu_tree();
    let items = &tree.iter().find(|(label, _)| *label == title).unwrap().1;
    let items = child.map_or(items.as_slice(), |name| {
        items
            .iter()
            .find_map(|item| match item {
                crate::menus::Item::Sub(label, children) if *label == name => Some(children.as_slice()),
                _ => None,
            })
            .unwrap()
    });
    let mut clicked = None;
    popup(ui, |ui| {
        crate::widgets::menu_scroll(ui, |ui| {
            ui.set_min_width(230.0);
            crate::menus::render_items(app, ui, items, true, &mut clicked);
        });
    });
    assert!(clicked.is_none(), "Soon must never dispatch");
}
fn popup(ui: &mut Ui, content: impl FnOnce(&mut Ui)) {
    egui::Popup::new(Id::unique("soon-menu-frame"), ui.ctx().clone(), egui::pos2(4.0, 4.0), ui.layer_id())
        .kind(egui::PopupKind::Menu)
        .layout(egui::Layout::top_down_justified(egui::Align::Min))
        .style(egui::containers::menu::menu_style)
        .show(content);
}
fn draw(ui: &mut Ui, app: &mut VectorcraftApp, surface: Surface) {
    ui.painter().rect_filled(ui.max_rect(), 0.0, Tokens::get(ui.ctx()).panel);
    match surface {
        Surface::Ask | Surface::Images | Surface::History => {
            ui.set_width(320.0);
            crate::varos::ai::show(app, ui);
        }
        Surface::Settings => crate::prefs_dialog::show(app, ui.ctx()),
        Surface::Object => menu(ui, app, "Object", None),
        Surface::Plugins => menu(ui, app, "Object", Some("Plug-ins")),
        Surface::Dieline => menu(ui, app, "Object", Some("Dieline")),
        Surface::Window => menu(ui, app, "Window", None),
        Surface::Setup => crate::dialogs::show(app, ui.ctx()),
        Surface::Color => crate::panels::separations::show(app, ui),
        Surface::Home => crate::varos::home::show(app, ui, ui.max_rect()),
        Surface::Links => popup(ui, |ui| crate::panels::links::menu(app, ui)),
        Surface::Share | Surface::Legacy => {
            app.fonts_ready = true;
            app.canvas.worker_started = true;
            app.ui(ui);
        }
    }
}
fn harness(surface: Surface) -> Harness<'static, VectorcraftApp> {
    Harness::builder()
        // Existing Color Settings combo boxes lack accessible names; this suite explicitly
        // checks every new Soon button's name and disabled state instead.
        .with_accessibility_check(false)
        .with_size(vec2(1440.0, 1200.0))
        .with_context_setup(move |ctx| setup(ctx, surface))
        .build_ui_state(move |ui, app| draw(ui, app, surface), app(surface))
}
fn texts(h: &Harness<'_, VectorcraftApp>) -> Vec<String> {
    fn collect(shape: &Shape, texts: &mut Vec<String>) {
        match shape {
            Shape::Text(t) => texts.push(t.galley.text().to_string()),
            Shape::Vec(v) => v.iter().for_each(|s| collect(s, texts)),
            _ => {}
        }
    }
    let mut result = vec![];
    for shape in &h.output().shapes {
        collect(&shape.shape, &mut result);
    }
    result
}
fn snapshot(app: &VectorcraftApp) -> (Vec<u8>, String, String, String, String) {
    let st = app.session.active().unwrap();
    (
        vectorcraft_format::save(&st.doc, false),
        format!("{:?}", st.history),
        format!("{:?}", st.selection),
        app.session.tool_id().into(),
        app.ui.status.clone(),
    )
}

#[test]
fn every_soon_surface_is_disabled_has_its_tooltip_and_physical_click_has_no_effect() {
    for (surface, labels) in SURFACES {
        let mut h = harness(*surface);
        h.run_steps(4);
        for label in *labels {
            let before = snapshot(h.state());
            let node = h.query_all_by_label(label).next().unwrap_or_else(|| panic!("{surface:?}: missing {label}"));
            assert!(node.accesskit_node().is_disabled(), "{surface:?}: {label}");
            node.hover();
            h.run_steps(5);
            if *label == "Dieline" {
                assert!(texts(&h).iter().any(|s| s == "Make Dieline…"), "the real disabled parent opens its submenu on hover");
            }
            assert!(texts(&h).iter().any(|s| s == &hover(label)), "{surface:?}: missing tooltip for {label}: {:?}", texts(&h));
            h.query_all_by_label(label).next().unwrap().click();
            h.run_steps(4);
            assert_eq!(snapshot(h.state()), before, "{surface:?}: {label} changed document/history/selection/tool/status");
        }
    }
}

#[test]
fn dieline_submenu_uses_intrinsic_width_and_opens_to_the_right() {
    let mut h = harness(Surface::Object);
    h.run_steps(4);
    let parent = h.get_by_label("Dieline").rect();
    assert!((200.0..350.0).contains(&parent.width()), "root menu must size to content: {parent:?}");
    h.get_by_label("Dieline").hover();
    h.run_steps(5);
    let child = h.get_by_label("Make Dieline…").rect();
    assert!((190.0..350.0).contains(&child.width()), "child must not consume viewport width: {child:?}");
    assert!(child.left() >= parent.right(), "submenu must open to the right when there is room: {parent:?}, {child:?}");
    h.run_steps(4);
    assert_eq!(h.get_by_label("Make Dieline…").rect(), child, "settled menu geometry must stay stable");
    assert!(texts(&h).contains(&hover("Dieline")), "parent tooltip survives the open submenu");
    let before = snapshot(h.state());
    h.get_by_label("Dieline").click();
    h.run_steps(4);
    assert_eq!(snapshot(h.state()), before);
    h.get_by_label("Make Dieline…").hover();
    h.run_steps(5);
    assert!(texts(&h).contains(&hover("Make Dieline…")), "child tooltip appears inside the real submenu");
    assert!(!texts(&h).contains(&hover("Dieline")), "parent tooltip ends when hovering the child");
}

#[test]
fn ask_composer_and_send_are_disabled_and_cannot_change_the_document() {
    let mut h = harness(Surface::Ask);
    h.run_steps(4);
    let before = snapshot(h.state());
    let rows: Vec<_> = h.query_all_by_label("Chat with Codex / Claude Code").collect();
    assert_eq!(rows.len(), 2, "provider and inert composer");
    for node in rows {
        assert!(node.accesskit_node().is_disabled());
        node.click();
    }
    h.run_steps(4);
    let send_label = hover("Chat with Codex / Claude Code");
    let send = h.get_by_label(&send_label);
    assert!(send.accesskit_node().is_disabled());
    send.click();
    h.run_steps(4);
    assert_eq!(snapshot(h.state()), before);
}

#[test]
fn roadmap_menus_preserve_positions_and_never_expose_an_executable_command_or_shortcut() {
    let app = app(Surface::Ask);
    let entries = crate::menus::menu_entries(&app);
    for label in ["Make Dieline…", "Extension Library…", "3D Box Preview", "Comments", "Preflight"] {
        let item = entries.iter().find(|e| e.label == label).unwrap();
        assert!(!item.enabled && item.command.is_none() && item.shortcut.is_empty());
        assert!(!crate::menus::UI_COMMANDS.iter().any(|(_, name, ..)| *name == label));
    }
    let tree = crate::menus::menu_tree();
    let names = |title| {
        tree.iter()
            .find(|(name, _)| *name == title)
            .unwrap()
            .1
            .iter()
            .filter_map(|item| match item {
                crate::menus::Item::Cmd(l, ..) | crate::menus::Item::Todo(l, _) | crate::menus::Item::Sub(l, _) => Some(*l),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    for (title, sequence) in [
        ("Object", vec!["Create Trim Marks", "Dieline", "Flatten Transparency…"]),
        ("Window", vec!["3D Box Preview", "Actions", "AI", "Align"]),
        ("Window", vec!["Color Themes", "Comments", "CSS Properties"]),
        ("Window", vec!["Pattern Options", "Preflight", "Properties"]),
    ] {
        assert!(names(title).windows(sequence.len()).any(|window| window == sequence), "{title}: {sequence:?}");
    }
    assert_eq!(crate::shortcut_editor::tool_shortcut("scissors"), Some("C"));
    for icon in ["frame", "chevron-down", "sparkles", "dc-arrow-up"] {
        assert!(crate::icons::exists(icon), "{icon}");
    }
    for item in ITEMS {
        assert!(item.icon.is_empty() || crate::icons::exists(item.icon), "{}: {}", item.label, item.icon);
    }
}

#[test]
fn ai_default_is_right_docked_but_saved_workspace_round_trips_unchanged() {
    use crate::varos::panel_system::{
        State,
        model::{Edge, Layout},
    };
    let fresh = State::default();
    let ai = fresh.layout.owner("ai").unwrap();
    assert!(!ai.hidden && ai.dock == Some(Edge::Right));
    let saved = Layout::new_workspace();
    assert!(saved.owner("ai").unwrap().hidden);
    let mut old = serde_json::to_value(&fresh).unwrap();
    old["layout"] = serde_json::to_value(&saved).unwrap();
    old["layout"]["hosts"].as_array_mut().unwrap().retain(|host| host["root"]["panels"].as_array().is_none_or(|p| !p.contains(&json!("ai"))));
    let state: State = serde_json::from_value(old.clone()).unwrap();
    assert_eq!(serde_json::to_value(&state).unwrap(), old);
}

#[test]
fn ai_crop_and_getting_started_invoke_the_existing_menu_commands() {
    let mut h = harness(Surface::Images);
    h.run_steps(3);
    assert!(h.get_by_role_and_label(egui::accesskit::Role::Button, "Crop Image").accesskit_node().is_disabled());
    let image = image::DynamicImage::new_rgba8(20, 20);
    let mut png = std::io::Cursor::new(Vec::new());
    image.write_to(&mut png, image::ImageFormat::Png).unwrap();
    h.state_mut()
        .run("file.place", json!({"name":"synthetic.png", "dataBase64":vectorcraft_format::base64_encode(png.get_ref()), "link":false}))
        .unwrap();
    h.run_steps(3);
    assert!(crate::menus::enabled(h.state(), "ui.cropImage"));
    let before = snapshot(h.state());
    h.get_by_role_and_label(egui::accesskit::Role::Button, "Crop Image").click();
    h.run_steps(3);
    assert_eq!(h.state().session.tool_id(), "cropImage");
    assert_eq!(snapshot(h.state()).0, before.0);
    assert_eq!(snapshot(h.state()).1, before.1);
    let mut h = harness(Surface::Ask);
    h.get_by_role_and_label(egui::accesskit::Role::Button, "Getting started").click();
    h.run_steps(3);
    assert_eq!(h.state().ui.home_view.section, crate::varos::home::Section::GettingStarted);
}

#[test]
fn native_soon_metadata_never_disables_a_working_command_with_a_matching_label() {
    use crate::{
        menus::Item as M,
        native_menu::{self, Node},
    };
    let app = app(Surface::Ask);
    let tree = vec![("Test", vec![M::Cmd("Comments", "shape.rectangle", serde_json::Value::Null), M::Todo("Comments", "")])];
    let native = native_menu::from_tree(&app, &tree);
    let rows = &native.menus[0].children;
    assert!(matches!(&rows[0],Node::Item(item) if item.command.is_some() && item.label=="Comments"));
    assert!(native_menu::soon_tooltip(&rows[0]).is_none());
    assert!(
        matches!(&rows[1],Node::Item(item) if item.command.is_none() && !item.enabled && item.shortcut.is_none() && item.label=="Comments — Soon")
    );
    assert_eq!(native_menu::soon_tooltip(&rows[1]), Some(hover("Comments")));
    let layout = native_menu::layout(&app);
    fn ai_settings(nodes: &[Node]) -> bool {
        nodes.iter().any(|node| match node {
            Node::Item(item) => item.command == Some("edit.preferences") && item.params["category"] == "AI",
            Node::Submenu { children, .. } => ai_settings(children),
            _ => false,
        })
    }
    assert!(layout.bar.menus.iter().any(|menu| ai_settings(&menu.children)));
}

#[test]
#[ignore = "software evidence only; set VAROS_SOON_CAPTURE_DIR"]
fn capture_soon_surfaces() {
    let path = std::path::PathBuf::from(std::env::var_os("VAROS_SOON_CAPTURE_DIR").expect("VAROS_SOON_CAPTURE_DIR required"));
    // Cargo runs unit tests from the crate; relative reviewer paths are workspace-relative.
    let path = if path.is_absolute() { path } else { std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").join(path) };
    std::fs::create_dir_all(&path).unwrap();
    for (surface, name, size) in [
        (Surface::Ask, "ai-ask", vec2(340.0, 740.0)),
        (Surface::Images, "ai-images", vec2(340.0, 620.0)),
        (Surface::History, "ai-history", vec2(340.0, 400.0)),
        (Surface::Settings, "settings-ai", vec2(1000.0, 760.0)),
        (Surface::Object, "object-menu", vec2(400.0, 1300.0)),
        (Surface::Plugins, "plugins-menu", vec2(340.0, 250.0)),
        (Surface::Dieline, "dieline-menu", vec2(340.0, 150.0)),
        (Surface::Window, "window-menu", vec2(400.0, 1700.0)),
        (Surface::Home, "home-templates", vec2(1440.0, 900.0)),
        (Surface::Links, "links-menu", vec2(460.0, 1000.0)),
        (Surface::Setup, "document-setup", vec2(900.0, 700.0)),
        (Surface::Color, "color-settings", vec2(460.0, 800.0)),
        (Surface::Share, "editor-share", vec2(1440.0, 900.0)),
        (Surface::Legacy, "editor-legacy-ai", vec2(1440.0, 900.0)),
        (Surface::Images, "ai-images-crop-enabled", vec2(340.0, 620.0)),
        (Surface::Object, "object-dieline-hover", vec2(800.0, 1300.0)),
        (Surface::Dieline, "dieline-tooltip", vec2(540.0, 260.0)),
    ] {
        let ctx = egui::Context::default();
        setup(&ctx, surface);
        let mut app = app(surface);
        if name == "ai-images-crop-enabled" {
            let mut png = std::io::Cursor::new(Vec::new());
            image::DynamicImage::new_rgba8(20, 20).write_to(&mut png, image::ImageFormat::Png).unwrap();
            app.run("file.place", json!({"name":"synthetic.png", "dataBase64":vectorcraft_format::base64_encode(png.get_ref()), "link":false}))
                .unwrap();
        }
        let hover_label = match name {
            "object-dieline-hover" => Some("Dieline"),
            "dieline-tooltip" => Some("Make Dieline…"),
            _ => None,
        };
        let mut pointer = None;
        let mut textures = std::collections::HashMap::<egui::TextureId, egui::ColorImage>::new();
        let mut shot = vec![];
        for step in 0..8 {
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                    time: Some(step as f64 * 0.1),
                    max_texture_side: Some(8192),
                    events: pointer.map(egui::Event::PointerMoved).into_iter().collect(),
                    ..Default::default()
                },
                |ui| draw(ui, &mut app, surface),
            );
            if let Some(label) = hover_label {
                fn center(shape: &Shape, label: &str) -> Option<Pos2> {
                    match shape {
                        Shape::Text(t) if t.galley.text() == label => Some(t.visual_bounding_rect().center()),
                        Shape::Vec(shapes) => shapes.iter().find_map(|shape| center(shape, label)),
                        _ => None,
                    }
                }
                pointer = out.shapes.iter().find_map(|shape| center(&shape.shape, label));
            }
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
        let pixels = crate::varos::home::capture::raster(&shot, &textures, size.x as u32, size.y as u32);
        image::save_buffer(path.join(format!("{name}.png")), &pixels, size.x as u32, size.y as u32, image::ColorType::Rgba8).unwrap();
    }
}
