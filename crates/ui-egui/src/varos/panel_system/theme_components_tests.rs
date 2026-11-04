//! Verify the real document and panel consumers, not just the theme helper.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use super::test_support::*;
use crate::varos::theme_components::FOUNDATION;

fn text_shapes(output: &FullOutput) -> Vec<&egui::epaint::TextShape> {
    fn collect<'a>(shape: &'a Shape, out: &mut Vec<&'a egui::epaint::TextShape>) {
        match shape {
            Shape::Text(text) => out.push(text),
            Shape::Vec(shapes) => shapes.iter().for_each(|s| collect(s, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for shape in &output.shapes {
        collect(&shape.shape, &mut out);
    }
    out
}

#[test]
fn actual_document_and_panel_tabs_use_the_same_typography_definition() {
    let mut h = Harness::workspace();
    h.app.run("file.new", json!({"name": "Inactive typography", "width":300,"height":300})).unwrap();
    h.app.run("file.new", json!({"name": "Shared typography عربي", "width":300,"height":300})).unwrap();
    h.app.run("ui.panelReview.action", json!({"action": Action::Activate { panel: "layers".into() }})).unwrap();
    h.frame(vec![]);
    let output = h.frame(vec![]);
    let texts = text_shapes(&output);
    let document = texts.iter().find(|text| text.galley.text().starts_with("Shared typography عربي")).expect("actual document tab");
    let panel = texts.iter().find(|text| text.galley.text() == "Layers").expect("actual Layers tab");
    let expected = FOUNDATION.tab_font();
    let t = crate::theme::Tokens::get(&h.ctx);
    let inactive = texts.iter().find(|text| text.galley.text().starts_with("Inactive typography")).expect("inactive document tab");
    assert!(inactive.galley.job.sections.iter().all(|section| section.format.color == t.text_dim));
    for text in [*document, *panel] {
        assert!(text.galley.job.sections.iter().all(|section| section.format.color == t.text));
    }
    for text in [*document, *panel] {
        assert!(!text.galley.job.sections.is_empty());
        for section in &text.galley.job.sections {
            assert_eq!(section.format.font_id, expected, "{} must use shared tab typography", text.galley.text());
        }
    }
}

#[test]
fn actual_panel_tab_hit_width_matches_the_shared_painted_font_and_padding() {
    let mut h = Harness::workspace();
    h.frame(vec![]);
    let label = super::paint::label("layers");
    let text_width = h.ctx.fonts_mut(|fonts| fonts.layout_no_wrap(label.into(), FOUNDATION.tab_font(), egui::Color32::PLACEHOLDER).size().x);
    let expected = (text_width.ceil() + 2.0 * FOUNDATION.tab_padding + 2.0 * super::metrics::BORDER)
        .clamp(super::metrics::TAB_MIN_WIDTH, super::metrics::TAB_MAX_WIDTH);
    near(h.hit("tab:layers").width(), expected);
}

#[test]
#[ignore = "optional software visual capture; set VAROS_SHARED_TABS_CAPTURE"]
fn capture_shared_document_and_panel_tabs() {
    let Some(path) = std::env::var_os("VAROS_SHARED_TABS_CAPTURE") else { return };
    let mut h = Harness::workspace();
    h.app.run("file.new", json!({"name": "Artwork عربي", "width":300,"height":300})).unwrap();
    h.app.run("file.new", json!({"name": "Second drawing", "width":300,"height":300})).unwrap();
    let pixels = super::ui_tests::software_capture(&mut h.app, 1600, 1000);
    image::save_buffer(path, &pixels, 1600, 1000, image::ColorType::Rgba8).unwrap();
}
