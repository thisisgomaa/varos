//! Contextual clipping action shared by Properties and the control bar.
use crate::VectorcraftApp;
use vectorcraft_doc::NodeKind;

pub(crate) struct Action {
    pub label: &'static str,
    pub command: &'static str,
    pub enabled: bool,
    pub tip: String,
}

pub(crate) fn action(app: &VectorcraftApp) -> Action {
    let mut a = Action {
        label: "Make Clipping Mask",
        command: "object.clippingMask.make",
        enabled: false,
        tip: "Select at least two objects; the top shape becomes the mask.".into(),
    };
    let Some(st) = app.session.active() else { return a };
    let mut roots = st
        .selection
        .in_paint_order(&st.doc)
        .into_iter()
        .map(|id| st.doc.parent_of(id).and_then(|p| st.doc.node(p)).filter(|n| matches!(n.kind, NodeKind::Compound { .. })).map_or(id, |n| n.id))
        .collect::<Vec<_>>();
    roots.dedup();
    let root_set: std::collections::HashSet<_> = roots.iter().copied().collect();
    let ids: Vec<_> = roots
        .iter()
        .copied()
        .filter(|id| {
            if st.doc.node(*id).is_none_or(|n| n.is_layer()) {
                return false;
            }
            let mut parent = st.doc.parent_of(*id);
            while let Some(p) = parent {
                if root_set.contains(&p) {
                    return false;
                }
                parent = st.doc.parent_of(p);
            }
            true
        })
        .collect();
    if ids.iter().any(|id| st.doc.node(*id).is_some_and(|n| n.clips())) {
        return Action {
            label: "Release Clipping Mask",
            command: "object.clippingMask.release",
            enabled: true,
            tip: "Release the selected clipping mask; keep its contents.".into(),
        };
    }
    if ids.len() < 2 {
        return a;
    }
    let Some(top) = ids.last().and_then(|id| st.doc.node(*id)) else { return a };
    a.enabled = matches!(top.kind, NodeKind::Path { guide: false, .. } | NodeKind::Compound { .. } | NodeKind::Text(_));
    a.tip = if a.enabled {
        format!("Mask: {} ({}), object {} — topmost in stacking order.", top.name.as_deref().unwrap_or("Unnamed"), top.kind_label(), top.id.0)
    } else {
        "The top object must be a path, compound path or text shape.".into()
    };
    a
}

pub(crate) fn button(app: &mut VectorcraftApp, ui: &mut egui::Ui) {
    let a = action(app);
    let response = ui.add_enabled(a.enabled, egui::Button::new(tl!(a.label)).corner_radius(3).min_size(egui::vec2(156., 24.)));
    if response.on_hover_text(&a.tip).on_disabled_hover_text(&a.tip).clicked()
        && let Err(error) = app.run(a.command, serde_json::json!({}))
    {
        app.status(error);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn clipping_action_tracks_selection_and_history_and_button_runs_command() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        app.run("file.new", json!({})).unwrap();
        assert!(!action(&app).enabled);
        let first = app.run("shape.rectangle", json!({"x":0,"y":0,"width":40,"height":40})).unwrap()["id"].clone();
        assert!(!action(&app).enabled);
        let second = app.run("shape.rectangle", json!({"x":0,"y":0,"width":20,"height":20})).unwrap()["id"].clone();
        app.run("select.set", json!({"ids":[first,second]})).unwrap();
        assert!(action(&app).enabled);
        assert!(action(&app).tip.contains(&format!("object {}", second)));
        let mut h = egui_kittest::Harness::new_ui(|ui| button(&mut app, ui));
        use egui_kittest::kittest::Queryable;
        h.get_by_label("Make Clipping Mask").click();
        h.run();
        drop(h);
        assert_eq!(action(&app).label, "Release Clipping Mask");
        app.run("edit.undo", json!({})).unwrap();
        assert_eq!(action(&app).label, "Make Clipping Mask");
        app.run("edit.redo", json!({})).unwrap();
        assert_eq!(action(&app).label, "Release Clipping Mask");
        let mut h = egui_kittest::Harness::new_ui(|ui| button(&mut app, ui));
        h.get_by_label("Release Clipping Mask").click();
        h.run();
        drop(h);
        assert_eq!(action(&app).label, "Make Clipping Mask");
        assert!(!action(&app).enabled);
    }

    #[test]
    fn properties_has_one_disabled_clipping_action_without_selection() {
        fn count(shape: &egui::Shape) -> usize {
            match shape {
                egui::Shape::Text(t) => usize::from(t.galley.text() == "Make Clipping Mask"),
                egui::Shape::Vec(shapes) => shapes.iter().map(count).sum(),
                _ => 0,
            }
        }
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        app.run("file.new", json!({"width":200,"height":200})).unwrap();
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let mut output = ctx.run_ui(
            egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(340., 1400.))), ..Default::default() },
            |ui| crate::panels::properties::show(&mut app, ui),
        );
        output.textures_delta.clear(); // This headless test inspects shapes, not GPU uploads.
        assert_eq!(output.shapes.iter().map(|s| count(&s.shape)).sum::<usize>(), 1);
        assert!(!action(&app).enabled);
    }

    #[test]
    fn group_on_top_is_disabled_with_reason() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        app.run("file.new", json!({})).unwrap();
        let art = app.run("shape.rectangle", json!({"x":0,"y":0,"width":40,"height":40})).unwrap()["id"].clone();
        let a = app.run("shape.rectangle", json!({"x":0,"y":0,"width":20,"height":20})).unwrap()["id"].clone();
        let b = app.run("shape.rectangle", json!({"x":0,"y":0,"width":10,"height":10})).unwrap()["id"].clone();
        app.run("select.set", json!({"ids":[a,b]})).unwrap();
        let group = app.run("object.group", json!({})).unwrap()["id"].clone();
        app.run("select.set", json!({"ids":[art,group]})).unwrap();
        assert!(!action(&app).enabled);
        assert!(action(&app).tip.contains("top object"));
    }
}
