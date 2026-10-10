//! Lane G: real kit emission on a CPU egui frame. No GPU, EventLoop or native window.
use accesskit::Role;
use egui::{Context, Id, Rect};
use varos_app::shell::{
    kit::{self, field, Control, Icon, IconState},
    tokens as t,
};
#[test]
fn every_interactive_kit_family_emits_role_name_and_values() {
    let ctx = Context::default();
    ctx.enable_accesskit();
    varos_app::shell::fonts::install(&ctx);
    let mut expected = vec![];
    let out = ctx.run_ui(egui::RawInput::default(), |ui| {
        for (key, name, kind) in
            [("action", "فتح", 0), ("list", "Document", 1), ("document", "Recent", 2), ("menu", "Check for Updates", 3)]
        {
            let c = Control::new(Id::new(key), name);
            let response = match kind {
                0 => kit::action(ui, c, false),
                1 => kit::list_row(ui, c, "/tmp/a"),
                2 => kit::document_row(ui, c, "/tmp/b", "Today"),
                _ => kit::menu_row(ui, c),
            };
            expected.push((
                response.response.id,
                name,
                match kind {
                    1 | 2 => Role::ListBoxOption,
                    3 => Role::MenuItem,
                    _ => Role::Button,
                },
            ));
        }
        for (key, name, state, role) in [
            ("button", "Add", IconState::Action, Role::Button),
            ("toggle", "Show grid", IconState::Toggle(true), Role::CheckBox),
            ("tool", "Select", IconState::Tool(true), Role::RadioButton),
        ] {
            let r = kit::icon_button(ui, Id::new(key), Icon::Plus, name, state);
            expected.push((r.response.id, name, role));
        }
        let _ = kit::text_dropdown(ui, Id::new("choice"), "A4", &["A4", "A3"], 160., "Page size");
        let number = field::number_field(
            ui,
            field::NumberField {
                id: Id::new("number"),
                width: 160.,
                label: field::Label::Letter("W"),
                tip: "Width",
                value: 42.,
                decimals: 1,
                speed: 1.,
                range: 0.1..=100.,
                disabled: false,
            },
        );
        expected.push((number.id, "Width", Role::SpinButton));
        let text = field::text_field(
            ui,
            field::TextField {
                id: Id::new("text"),
                rect: Rect::from_min_size(egui::pos2(0., 400.), egui::vec2(160., 26.)),
                value: "مرحبا",
                font: t::small(),
                framed: true,
                open: false,
                hint: "Name",
            },
            |s| Ok(s.to_owned()),
        );
        expected.push((text.id, "Name", Role::TextInput));
        let rect = Rect::from_min_size(egui::pos2(200., 400.), egui::vec2(160., 26.));
        kit::board::table_row(ui, Id::new("board"), rect, false, "Board");
        expected.push((Id::new("board"), "Board", Role::Button));
        varos_app::shell::accessibility::emit(ui, Id::new("panel"), Role::Pane, "Properties", None, true);
        expected.push((Id::new("panel"), "Properties", Role::Pane));
        kit::section_heading(ui, "Updates");
        kit::notice(ui, "Ready");
        kit::text(ui, "Text label", t::small(), t::TEXT);
    });
    let tree = out.platform_output.accesskit_update.expect("enabled tree");
    for (id, name, role) in expected {
        let node = &tree.nodes.iter().find(|(n, _)| *n == id.accesskit_id()).expect(name).1;
        assert_eq!(node.role(), role, "{name}");
        assert_eq!(node.label(), Some(name));
    }
    for name in ["Updates", "Ready", "Text label"] {
        assert!(tree.nodes.iter().any(|(_, n)| n.label() == Some(name)), "{name}");
    }
    let number = &tree.nodes.iter().find(|(id, _)| *id == Id::new("number").accesskit_id()).unwrap().1;
    assert_eq!(number.value(), Some("42.0"));
    let text = &tree.nodes.iter().find(|(id, _)| *id == Id::new("text").accesskit_id()).unwrap().1;
    assert_eq!(text.value(), Some("مرحبا"));
}
#[test]
fn accessibility_click_is_single_activation_and_disabled_controls_refuse() {
    let ctx = Context::default();
    ctx.enable_accesskit();
    varos_app::shell::fonts::install(&ctx);
    let input = egui::RawInput {
        events: vec![egui::Event::AccessKitActionRequest(accesskit::ActionRequest {
            action: accesskit::Action::Click,
            target_tree: accesskit::TreeId::ROOT,
            target_node: Id::new("button").accesskit_id(),
            data: None,
        })],
        ..Default::default()
    };
    let mut hit = false;
    let _ = ctx.run_ui(input, |ui| {
        hit = kit::action(ui, Control::new(Id::new("button"), "Open"), false).activated;
    });
    assert!(hit);
}
#[test]
fn board_controls_and_static_chips_have_accessible_names() {
    use kit::board as b;
    let ctx = Context::default();
    ctx.enable_accesskit();
    varos_app::shell::fonts::install(&ctx);
    let out = ctx.run_ui(egui::RawInput::default(), |ui| {
        let rect = Rect::from_min_size(egui::pos2(50., 50.), egui::vec2(300., 100.));
        let label = ui.painter().layout_no_wrap("Label".into(), t::small(), t::TEXT);
        b::big_button(
            ui,
            Id::new("big"),
            rect,
            b::BigButton {
                icon: Icon::Plus,
                title: label.clone(),
                sub: label.clone(),
                shortcut: label.clone(),
                primary: true,
                focused: false,
            },
            "New document",
        );
        b::text_button(
            ui,
            Id::new("textbutton"),
            rect,
            label.clone(),
            b::ButtonKind::Solid,
            kit::Availability::Enabled,
            false,
            "Recover",
        );
        b::filter_tab(ui, Id::new("filter"), rect, label.clone(), label.clone(), true, false, "All boards");
        b::segmented(ui, Id::new("segment"), rect, &[(Icon::Plus, "Grid"), (Icon::Plus, "List")], 0, None);
        b::preset_cell(
            ui,
            Id::new("preset"),
            rect,
            egui::vec2(20., 20.),
            (label.clone(), 100.),
            (label.clone(), 110.),
            false,
            "A4 preset",
        );
        b::card(ui, Id::new("card"), rect, rect, b::CardState::default(), "Document card");
        b::more_chip(ui, Id::new("more"), rect, false);
        b::tag_chip(ui, Id::new("tagchip"), rect, label.clone(), "Remove tag");
        for (i, name) in ["Tag", "Missing", "Command key"].iter().enumerate() {
            let r = rect.translate(egui::vec2(0., i as f32 * 100.));
            let g = ui.painter().layout_no_wrap((*name).into(), t::small(), t::TEXT);
            match i {
                0 => b::tag_pill(ui.painter(), r, g),
                1 => b::outline_pill(ui.painter(), r, g),
                _ => b::kbd(ui.painter(), r, g),
            }
        }
    });
    let tree = out.platform_output.accesskit_update.unwrap();
    for name in [
        "New document",
        "Recover",
        "All boards",
        "Grid",
        "List",
        "A4 preset",
        "Document card",
        "Board actions",
        "Remove tag",
        "Tag",
        "Missing",
        "Command key",
    ] {
        assert!(tree.nodes.iter().any(|(_, n)| n.label() == Some(name) && n.role() != Role::Unknown), "{name}");
    }
}
#[test]
fn arabic_accessible_name_comes_from_logical_varos_text_label_source() {
    use varos_text::{Engine, FaceId, FallbackPolicy, FontFace, FontSet, LabelCache, LabelKey};
    let fonts = FontSet::new(
        vec![FontFace::new(
            "IBM Plex Sans Arabic",
            400,
            include_bytes!("../assets/fonts/IBMPlexSansArabic-Regular.ttf").as_slice().into(),
        )
        .unwrap()],
        FallbackPolicy { common: vec![FaceId(0)], scripts: vec![] },
    )
    .unwrap();
    let mut engine = Engine::new(fonts).unwrap();
    let mut labels = LabelCache::new(&engine);
    let key = LabelKey {
        text: "فتح المستند".into(),
        face: FaceId(0),
        size_bits: 14f32.to_bits(),
        width_bits: None,
        role: 0,
        ppp_bits: 1f32.to_bits(),
    };
    let layout = labels.layout(&mut engine, &key, 1).unwrap();
    let ctx = Context::default();
    ctx.enable_accesskit();
    let output = ctx.run_ui(egui::RawInput::default(), |ui| {
        kit::action(ui, Control::new(Id::new("arabic"), &layout.source), false);
    });
    assert_eq!(
        output
            .platform_output
            .accesskit_update
            .unwrap()
            .nodes
            .iter()
            .find(|(id, _)| *id == Id::new("arabic").accesskit_id())
            .unwrap()
            .1
            .label(),
        Some("فتح المستند")
    );
}
#[test]
fn configured_update_trust_survives_preferences_save() {
    use varos_app::storage::{durable::RealFs, settings::Settings};
    let root = std::env::temp_dir().join(format!("varos-lane-g-settings-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("settings.json");
    let mut value = serde_json::to_value(Settings::default()).unwrap();
    value["version"] = serde_json::json!(2);
    value["update_manifest_url"] = serde_json::json!("https://example.org/manifest.json");
    value["update_public_key"] = serde_json::json!("trusted key");
    std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    let settings = Settings { recovery_enabled: false, ..Settings::default() };
    settings.save(&RealFs, &path).unwrap();
    let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(saved["update_manifest_url"], value["update_manifest_url"]);
    assert_eq!(saved["update_public_key"], value["update_public_key"]);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn pointer_only_menu_activation_and_disabled_refusal() {
    let ctx = Context::default();
    ctx.enable_accesskit();
    let event = || {
        egui::Event::AccessKitActionRequest(accesskit::ActionRequest {
            action: accesskit::Action::Click,
            target_tree: accesskit::TreeId::ROOT,
            target_node: Id::new("menu-action").accesskit_id(),
            data: None,
        })
    };
    for (disabled, events, expected) in
        [(false, vec![event()], true), (false, vec![], false), (true, vec![event()], false)]
    {
        let mut hit = false;
        let output = ctx.run_ui(egui::RawInput { events, ..Default::default() }, |ui| {
            let mut c = Control::new(Id::new("menu-action"), "Download");
            c.pointer_only = true;
            if disabled {
                c.availability = kit::Availability::Disabled("Not available");
            }
            hit = kit::menu_row(ui, c).activated;
        });
        assert_eq!(hit, expected);
        let tree = output.platform_output.accesskit_update.unwrap();
        assert_eq!(
            tree.nodes.iter().find(|(id, _)| *id == Id::new("menu-action").accesskit_id()).unwrap().1.is_disabled(),
            disabled
        );
    }
}
#[test]
fn named_focus_order_follows_kit_draw_order() {
    let ctx = Context::default();
    ctx.enable_accesskit();
    let draw = |ui: &mut egui::Ui| {
        for name in ["First", "Second", "Third"] {
            kit::action(ui, Control::new(Id::new(name), name), false);
        }
    };
    let output = ctx.run_ui(egui::RawInput::default(), draw);
    let tree = output.platform_output.accesskit_update.unwrap();
    fn walk(tree: &accesskit::TreeUpdate, id: accesskit::NodeId, names: &mut Vec<String>) {
        let Some((_, n)) = tree.nodes.iter().find(|(node, _)| *node == id) else { return };
        if let Some(label) = n.label().filter(|l| ["First", "Second", "Third"].contains(l)) {
            names.push(label.to_owned());
        }
        for child in n.children() {
            walk(tree, *child, names);
        }
    }
    let mut names = vec![];
    walk(&tree, tree.tree.as_ref().unwrap().root, &mut names);
    assert_eq!(names, ["First", "Second", "Third"]);
    let event = egui::Event::AccessKitActionRequest(accesskit::ActionRequest {
        action: accesskit::Action::Focus,
        target_tree: accesskit::TreeId::ROOT,
        target_node: Id::new("Second").accesskit_id(),
        data: None,
    });
    let output = ctx.run_ui(egui::RawInput { events: vec![event], ..Default::default() }, draw);
    assert_eq!(output.platform_output.accesskit_update.unwrap().focus, Id::new("Second").accesskit_id());
}
