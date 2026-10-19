//! One Home predicate for welcome content and editor chrome.
use crate::VectorcraftApp;

/// Home may cover open documents until their identity/count changes (New/Open/Close).
/// Ignore a stale Home key before chrome layout, so a new document gets chrome immediately.
pub(crate) fn is_home(app: &VectorcraftApp) -> bool {
    app.ui.home.is_some_and(|key| key == crate::menus::home_key(app)) || app.session.active().is_none()
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
            for label in ["New file", "Open", "Quickly start a new file"] {
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
