//! The side Color panel as Varos presents it: always upstream's compact sliders
//! (`panels::color::show`), measured so a flyout can fit them, with a Recent Colors strip the user
//! can hide; and a gradient stop's popover editor (the wheel picker, or the sliders with Classic).
//! The toolbar wells' wheel popover is separate and never shares this panel's state or size.
use crate::VectorcraftApp;
use egui::Ui;
use serde_json::{Value, json};
use vectorcraft_color::Color;

fn content_height_id() -> egui::Id {
    egui::Id::unique("color-panel-content-height")
}

/// The side Color panel's body: the sliders, with their measured content height recorded.
pub(crate) fn side(app: &mut VectorcraftApp, ui: &mut Ui) {
    let top = ui.cursor().top();
    crate::panels::color::show(app, ui);
    let height = ui.cursor().top() - top - ui.spacing().item_spacing.y;
    if height.is_finite() && height > 0.0 {
        let previous = content_height(ui.ctx());
        ui.ctx().data_mut(|data| data.insert_temp(content_height_id(), height.ceil()));
        if previous.is_none_or(|old| (old - height).abs() > 1.0) {
            ui.ctx().request_repaint();
        }
    }
}

/// The measured slider-panel contents; the popup wheel's preferences never affect it.
pub(crate) fn content_height(ctx: &egui::Context) -> Option<f32> {
    ctx.data(|data| data.get_temp::<f32>(content_height_id()))
}

/// A gradient stop's popover editor: the wheel picker (the sliders with Classic on). It edits only
/// that stop and leaves the side panel's measured height alone.
pub(crate) fn stop_editor(app: &mut VectorcraftApp, ui: &mut Ui) {
    if app.ui.varos_picker.classic {
        crate::panels::color::show(app, ui);
    } else {
        super::picker::show_stop(app, ui);
    }
}

/// The side panel's Recent Colors strip (compact chips, then a divider) unless the user hid it;
/// the colour clicked, for the panel to apply.
pub(crate) fn recent(app: &VectorcraftApp, ui: &mut Ui) -> Option<Color> {
    if app.ui.color_hide_recent {
        return None;
    }
    let picked = crate::panels::recent_colors_chips(app, ui, true);
    crate::widgets::divider(ui);
    picked
}

/// The panel menu's Show Recent Colors item.
pub(crate) fn menu(app: &mut VectorcraftApp, ui: &mut Ui) {
    if crate::widgets::menu_item(ui, tl!("Show Recent Colors"), true, !app.ui.color_hide_recent) {
        command(app, &json!({ "recent": app.ui.color_hide_recent })).ok();
    }
}

/// `ui.colorPanel {mode?, options?, recent?}`: upstream's command plus `recent`, the strip's
/// visibility (kept with the preferences). Every parameter is checked before anything changes:
/// upstream's `mode` and `options` first, by upstream's own check, so a request with several
/// malformed fields reports the same first error as upstream does; then `recent`.
pub(crate) fn command(app: &mut VectorcraftApp, p: &Value) -> Result<Value, String> {
    let params = crate::panels::color::params(p)?;
    let recent = crate::menus::opt_bool(p, "recent")?;
    let mut out = crate::panels::color::apply_params(app, params);
    if let Some(shown) = recent {
        app.ui.color_hide_recent = !shown;
    }
    if let Some(map) = out.as_object_mut() {
        map.insert("recent".into(), json!(!app.ui.color_hide_recent));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vectorcraft_engine::Session;

    fn height(app: &mut VectorcraftApp, ctx: &egui::Context) -> f32 {
        let mut height = 0.0;
        let mut out = ctx.run_ui(Default::default(), |ui| {
            crate::panels::color::show(app, ui);
            height = ui.min_rect().height();
        });
        out.textures_delta.clear();
        height
    }

    #[test]
    fn recent_colors_visibility_persists_independently_of_the_popup() {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.run("file.new", json!({"width": 200, "height": 200})).unwrap();
        app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 50, "height": 50})).unwrap();
        app.ui.varos_picker.drawer_open = true;
        let popup = app.ui.varos_picker.clone();
        let ctx = egui::Context::default();
        let full_height = height(&mut app, &ctx);
        assert_eq!(app.run("ui.colorPanel", json!({"recent":false})).unwrap()["recent"], false);
        let hidden_height = height(&mut app, &ctx);
        assert!(hidden_height < full_height, "hiding the strip removes its space");
        assert_eq!(app.ui.varos_picker, popup);
        let saved: crate::UiState = serde_json::from_value(serde_json::to_value(&app.ui).unwrap()).unwrap();
        assert!(saved.color_hide_recent);
        assert!(app.run("ui.colorPanel", json!({"recent":"no"})).is_err());
        assert!(app.ui.color_hide_recent);
        app.run("ui.colorPanel", json!({"recent":true})).unwrap();
        assert_eq!(height(&mut app, &ctx), full_height);
        assert_eq!(app.ui.varos_picker, popup);
    }

    #[test]
    fn a_malformed_recent_changes_nothing() {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        assert!(app.run("ui.colorPanel", json!({"mode": "hsb", "recent": "no"})).is_err());
        assert_eq!(app.ui.color_mode, None, "the mode is not picked either");
        assert!(app.run("ui.colorPanel", json!({"mode": "nope", "recent": false})).is_err());
        assert!(!app.ui.color_hide_recent, "nor is the strip hidden when the mode is malformed");
    }

    #[test]
    fn several_malformed_fields_report_upstreams_first_error() {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        for (p, upstream) in [
            (json!({"mode": "nope", "recent": "no"}), json!({"mode": "nope"})),
            (json!({"options": 3, "recent": "no"}), json!({"options": 3})),
            (json!({"mode": "nope", "options": 3, "recent": "no"}), json!({"mode": "nope", "options": 3})),
        ] {
            let expected = crate::panels::color::command(&mut app, &upstream).unwrap_err();
            assert_eq!(app.run("ui.colorPanel", p).unwrap_err(), expected);
        }
        assert_eq!(app.run("ui.colorPanel", json!({"mode": "hsb", "recent": "no"})).unwrap_err(), "`recent` must be true or false");
        assert_eq!(app.ui.color_mode, None, "a malformed recent alone still changes nothing");
    }
}
