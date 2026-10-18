//! Varos SVG overrides; unmatched names retain the upstream artwork.
//! File assets are copied verbatim; legacy glyphs are extracted from the owner-supplied registry.
use std::{
    collections::HashMap,
    sync::{Arc, OnceLock},
};

const ORIGINALS: &[(&str, &[u8])] = &[
    ("align-center-horizontal", include_bytes!("../../assets/varos/icons/legacy-al-m.svg")),
    ("align-center-vertical", include_bytes!("../../assets/varos/icons/legacy-al-ch.svg")),
    ("align-end-horizontal", include_bytes!("../../assets/varos/icons/legacy-al-b.svg")),
    ("align-end-vertical", include_bytes!("../../assets/varos/icons/legacy-al-r.svg")),
    ("align-horizontal-justify-start", include_bytes!("../../assets/varos/icons/align-horizontal-distribute-start.svg")),
    ("align-start-horizontal", include_bytes!("../../assets/varos/icons/legacy-al-t.svg")),
    ("align-start-vertical", include_bytes!("../../assets/varos/icons/legacy-al-l.svg")),
    ("arrow-left-right", include_bytes!("../../assets/varos/icons/varos-picker-swap.svg")),
    ("chevron-down", include_bytes!("../../assets/varos/icons/chevron-down.svg")),
    ("chevron-left", include_bytes!("../../assets/varos/icons/chevron-left.svg")),
    ("chevron-right", include_bytes!("../../assets/varos/icons/chevron-right.svg")),
    ("circle", include_bytes!("../../assets/varos/icons/varos-draw-ellipse.svg")),
    ("copy", include_bytes!("../../assets/varos/icons/copy.svg")),
    ("dc-al-bottom", include_bytes!("../../assets/varos/icons/legacy-al-b.svg")),
    ("dc-al-hcenter", include_bytes!("../../assets/varos/icons/legacy-al-ch.svg")),
    ("dc-al-left", include_bytes!("../../assets/varos/icons/legacy-al-l.svg")),
    ("dc-al-right", include_bytes!("../../assets/varos/icons/legacy-al-r.svg")),
    ("dc-al-top", include_bytes!("../../assets/varos/icons/legacy-al-t.svg")),
    ("dc-al-vcenter", include_bytes!("../../assets/varos/icons/legacy-al-m.svg")),
    ("dc-align", include_bytes!("../../assets/varos/icons/legacy-al-ch.svg")),
    ("dc-alignto-artboard", include_bytes!("../../assets/varos/icons/frame.svg")),
    ("dc-alignto-selection", include_bytes!("../../assets/varos/icons/square-dashed.svg")),
    ("dc-arc", include_bytes!("../../assets/varos/icons/varos-draw-arc.svg")),
    ("dc-artboards", include_bytes!("../../assets/varos/icons/frame.svg")),
    ("dc-color-guide", include_bytes!("../../assets/varos/icons/varos-picker-harmony.svg")),
    ("dc-direct", include_bytes!("../../assets/varos/icons/varos-draw-direct.svg")),
    ("dc-dist-bottom", include_bytes!("../../assets/varos/icons/align-vertical-distribute-end.svg")),
    ("dc-dist-hcenter", include_bytes!("../../assets/varos/icons/align-horizontal-distribute-center.svg")),
    ("dc-dist-hspace", include_bytes!("../../assets/varos/icons/align-horizontal-distribute-center.svg")),
    ("dc-dist-left", include_bytes!("../../assets/varos/icons/align-horizontal-distribute-start.svg")),
    ("dc-dist-right", include_bytes!("../../assets/varos/icons/align-horizontal-distribute-end.svg")),
    ("dc-dist-top", include_bytes!("../../assets/varos/icons/align-vertical-distribute-start.svg")),
    ("dc-dist-vcenter", include_bytes!("../../assets/varos/icons/align-vertical-distribute-center.svg")),
    ("dc-dist-vspace", include_bytes!("../../assets/varos/icons/align-vertical-distribute-center.svg")),
    ("dc-ellipse", include_bytes!("../../assets/varos/icons/varos-draw-ellipse.svg")),
    ("dc-fill-none", include_bytes!("../../assets/varos/icons/varos-picker-none.svg")),
    ("dc-folder", include_bytes!("../../assets/varos/icons/folder.svg")),
    ("dc-grad-linear", include_bytes!("../../assets/varos/icons/varos-picker-gradient.svg")),
    ("dc-gradient", include_bytes!("../../assets/varos/icons/varos-picker-gradient.svg")),
    ("dc-grid-view", include_bytes!("../../assets/varos/icons/layout-grid.svg")),
    ("dc-join", include_bytes!("../../assets/varos/icons/link.svg")),
    ("dc-knife", include_bytes!("../../assets/varos/icons/varos-path-knife.svg")),
    ("dc-line", include_bytes!("../../assets/varos/icons/varos-draw-line.svg")),
    ("dc-line-cut", include_bytes!("../../assets/varos/icons/varos-path-knife.svg")),
    ("dc-list-view", include_bytes!("../../assets/varos/icons/list.svg")),
    ("dc-mask-none", include_bytes!("../../assets/varos/icons/varos-picker-none.svg")),
    ("dc-measure", include_bytes!("../../assets/varos/icons/ruler.svg")),
    ("dc-new-fill", include_bytes!("../../assets/varos/icons/plus.svg")),
    ("dc-new-item", include_bytes!("../../assets/varos/icons/plus.svg")),
    ("dc-new-stroke", include_bytes!("../../assets/varos/icons/plus.svg")),
    ("dc-path-eraser", include_bytes!("../../assets/varos/icons/varos-path-eraser.svg")),
    ("dc-pathfinder", include_bytes!("../../assets/varos/icons/varos-path-merge.svg")),
    ("dc-pf-divide", include_bytes!("../../assets/varos/icons/varos-path-divide.svg")),
    ("dc-pf-merge", include_bytes!("../../assets/varos/icons/varos-path-merge.svg")),
    ("dc-pf-minus-back", include_bytes!("../../assets/varos/icons/varos-path-minus-back.svg")),
    ("dc-pf-outline", include_bytes!("../../assets/varos/icons/varos-path-outline.svg")),
    ("dc-pf-trim", include_bytes!("../../assets/varos/icons/varos-path-trim.svg")),
    ("dc-polar-grid", include_bytes!("../../assets/varos/icons/varos-draw-polar-grid.svg")),
    ("dc-polygon", include_bytes!("../../assets/varos/icons/varos-draw-polygon.svg")),
    ("dc-rect-cut", include_bytes!("../../assets/varos/icons/varos-path-knife.svg")),
    ("dc-rect-grid", include_bytes!("../../assets/varos/icons/layout-grid.svg")),
    ("dc-remove-brush", include_bytes!("../../assets/varos/icons/trash-2.svg")),
    ("dc-reverse", include_bytes!("../../assets/varos/icons/varos-picker-swap.svg")),
    ("dc-rounded-rect", include_bytes!("../../assets/varos/icons/varos-draw-rounded-rect.svg")),
    ("dc-selection", include_bytes!("../../assets/varos/icons/varos-draw-select.svg")),
    ("dc-shape-builder", include_bytes!("../../assets/varos/icons/varos-path-merge.svg")),
    ("dc-smooth", include_bytes!("../../assets/varos/icons/varos-draw-smooth.svg")),
    ("dc-stroke", include_bytes!("../../assets/varos/icons/legacy-strokew.svg")),
    ("dc-swap", include_bytes!("../../assets/varos/icons/varos-picker-swap.svg")),
    ("dc-swatch-kinds", include_bytes!("../../assets/varos/icons/list-filter.svg")),
    ("dc-transform-panel", include_bytes!("../../assets/varos/icons/legacy-scale.svg")),
    ("dc-transparency", include_bytes!("../../assets/varos/icons/legacy-opacity.svg")),
    ("dc-zoom-large", include_bytes!("../../assets/varos/icons/zoom-in.svg")),
    ("ellipsis", include_bytes!("../../assets/varos/icons/ellipsis.svg")),
    ("eraser", include_bytes!("../../assets/varos/icons/varos-path-eraser.svg")),
    ("eye", include_bytes!("../../assets/varos/icons/legacy-l-eye.svg")),
    ("eye-off", include_bytes!("../../assets/varos/icons/legacy-l-eyeoff.svg")),
    ("file-plus", include_bytes!("../../assets/varos/icons/file-plus.svg")),
    ("flip-horizontal-2", include_bytes!("../../assets/varos/icons/square-centerline-dashed-horizontal.svg")),
    ("flip-vertical-2", include_bytes!("../../assets/varos/icons/square-centerline-dashed-vertical.svg")),
    ("folder-open", include_bytes!("../../assets/varos/icons/folder-open.svg")),
    ("frame", include_bytes!("../../assets/varos/icons/frame.svg")),
    ("grid-3x3", include_bytes!("../../assets/varos/icons/grip.svg")),
    ("group", include_bytes!("../../assets/varos/icons/folder.svg")),
    ("hand", include_bytes!("../../assets/varos/icons/hand.svg")),
    ("history", include_bytes!("../../assets/varos/icons/history.svg")),
    ("house", include_bytes!("../../assets/varos/icons/house.svg")),
    ("lasso", include_bytes!("../../assets/varos/icons/lasso.svg")),
    ("layout-grid", include_bytes!("../../assets/varos/icons/layout-grid.svg")),
    ("link", include_bytes!("../../assets/varos/icons/link.svg")),
    ("link-2-off", include_bytes!("../../assets/varos/icons/link.svg")),
    ("lock", include_bytes!("../../assets/varos/icons/legacy-l-lock.svg")),
    ("lock-open", include_bytes!("../../assets/varos/icons/legacy-l-unlock.svg")),
    ("menu", include_bytes!("../../assets/varos/icons/menu.svg")),
    ("move", include_bytes!("../../assets/varos/icons/move.svg")),
    ("palette", include_bytes!("../../assets/varos/icons/varos-picker-wheel.svg")),
    ("pen-tool", include_bytes!("../../assets/varos/icons/pen-tool.svg")),
    ("pen-tool-add", include_bytes!("../../assets/varos/icons/pen-line.svg")),
    ("pen-tool-delete", include_bytes!("../../assets/varos/icons/pen-off.svg")),
    ("pencil", include_bytes!("../../assets/varos/icons/varos-draw-pencil.svg")),
    ("pipette", include_bytes!("../../assets/varos/icons/pipette.svg")),
    ("plus", include_bytes!("../../assets/varos/icons/plus.svg")),
    ("rotate-cw", include_bytes!("../../assets/varos/icons/legacy-rotate.svg")),
    ("ruler", include_bytes!("../../assets/varos/icons/ruler.svg")),
    ("scaling", include_bytes!("../../assets/varos/icons/legacy-scale.svg")),
    ("scissors", include_bytes!("../../assets/varos/icons/varos-path-scissors.svg")),
    ("search", include_bytes!("../../assets/varos/icons/legacy-l-search.svg")),
    ("spline", include_bytes!("../../assets/varos/icons/varos-draw-curvature.svg")),
    ("square", include_bytes!("../../assets/varos/icons/varos-draw-rect.svg")),
    ("square-dashed", include_bytes!("../../assets/varos/icons/square-dashed.svg")),
    ("star", include_bytes!("../../assets/varos/icons/varos-draw-star.svg")),
    ("tornado", include_bytes!("../../assets/varos/icons/varos-draw-spiral.svg")),
    ("trash-2", include_bytes!("../../assets/varos/icons/trash-2.svg")),
    ("triangle", include_bytes!("../../assets/varos/icons/varos-draw-triangle.svg")),
    ("type", include_bytes!("../../assets/varos/icons/legacy-type.svg")),
    ("x", include_bytes!("../../assets/varos/icons/x.svg")),
    ("zoom-in", include_bytes!("../../assets/varos/icons/zoom-in.svg")),
];

pub fn source(name: &str) -> Option<egui::ImageSource<'static>> {
    static TABLE: OnceLock<HashMap<&'static str, Arc<[u8]>>> = OnceLock::new();
    let table = TABLE.get_or_init(|| {
        ORIGINALS
            .iter()
            .map(|(name, bytes)| {
                // White is the neutral tint mask; match the upstream mixed-set stroke weight.
                // TODO: The owner may want 2.0 strokes later.
                let white = String::from_utf8_lossy(bytes).replace("currentColor", "white").replace("stroke-width=\"2\"", "stroke-width=\"1.6\"");
                (*name, Arc::from(white.into_bytes()))
            })
            .collect()
    });
    let bytes = table.get(name)?.clone();
    Some(egui::ImageSource::Bytes { uri: format!("bytes://varos/icon/{name}.svg").into(), bytes: egui::load::Bytes::Shared(bytes) })
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_override_rasterizes_and_missing_icons_fall_back() {
        assert_eq!(super::ORIGINALS.len(), 117);
        // Optional contact sheet for visual QA, using the same rasterizer as the app.
        let preview = std::env::var_os("VECTORCRAFT_ICON_ATLAS");
        let mut atlas = preview.as_ref().map(|_| image::RgbaImage::from_pixel(768, 640, image::Rgba([32, 32, 32, 255])));
        let mut seen = std::collections::HashSet::new();
        for (index, (name, original)) in super::ORIGINALS.iter().enumerate() {
            assert!(seen.insert(name), "duplicate override: {name}");
            assert!(crate::icons::exists(name), "unknown catalogue name: {name}");
            let Some(egui::ImageSource::Bytes { uri, bytes }) = super::source(name) else { panic!("missing override: {name}") };
            let egui::ImageSource::Bytes { uri: actual_uri, bytes: actual_bytes } = crate::icons::source(name) else {
                panic!("expected Varos SVG bytes")
            };
            assert_eq!(actual_uri, uri);
            assert_eq!(actual_bytes.as_ref(), bytes.as_ref());
            let expected = String::from_utf8_lossy(original).replace("currentColor", "white").replace("stroke-width=\"2\"", "stroke-width=\"1.6\"");
            assert_eq!(bytes.as_ref(), expected.as_bytes(), "{name}: upstream stroke/tint conventions");
            let image = egui_extras::image::load_svg_bytes(bytes.as_ref(), &Default::default()).unwrap();
            assert_eq!(image.size, [24, 24], "{name}");
            if let Some(atlas) = &mut atlas {
                for (pixel, color) in image.pixels.iter().enumerate() {
                    let gray = color.r().saturating_add(((255 - u16::from(color.a())) * 32 / 255) as u8);
                    let x = (index % 12 * 64 + 20 + pixel % 24) as u32;
                    let y = (index / 12 * 64 + 20 + pixel / 24) as u32;
                    atlas.put_pixel(x, y, image::Rgba([gray, gray, gray, 255]));
                }
            }
            assert!(image.pixels.iter().any(|p| p.a() > 0), "{name}: empty glyph");
            assert!(
                image.pixels.iter().filter(|p| p.a() > 0).all(|p| p.r() == p.a() && p.g() == p.a() && p.b() == p.a()),
                "{name}: must tint from white"
            );
        }
        if let (Some(atlas), Some(path)) = (atlas, preview) {
            atlas.save(path).unwrap();
        }
        for (name, original) in crate::icon_data::ICONS {
            if super::source(name).is_none() {
                let egui::ImageSource::Bytes { bytes, .. } = crate::icons::source(name) else { panic!("expected upstream SVG bytes") };
                let expected =
                    String::from_utf8_lossy(original).replace("currentColor", "white").replace("stroke-width=\"2\"", "stroke-width=\"1.6\"");
                assert_eq!(bytes.as_ref(), expected.as_bytes(), "{name}: fallback changed");
            }
        }
    }

    #[test]
    fn tool_aliases_resolve_to_catalogue_overrides() {
        for (alias, catalogue) in &[
            ("tool-arc", "dc-arc"),
            ("tool-artboard", "frame"),
            ("tool-curvature", "spline"),
            ("tool-direct", "dc-direct"),
            ("tool-ellipse", "dc-ellipse"),
            ("tool-eraser", "eraser"),
            ("tool-eyedropper", "pipette"),
            ("tool-gradient", "dc-gradient"),
            ("tool-hand", "hand"),
            ("tool-join", "dc-join"),
            ("tool-knife", "dc-knife"),
            ("tool-lasso", "lasso"),
            ("tool-line", "dc-line"),
            ("tool-line-cut", "dc-line-cut"),
            ("tool-measure", "dc-measure"),
            ("tool-path-eraser", "dc-path-eraser"),
            ("tool-pen", "pen-tool"),
            ("tool-pen-add", "pen-tool-add"),
            ("tool-pen-delete", "pen-tool-delete"),
            ("tool-pencil", "pencil"),
            ("tool-polar-grid", "dc-polar-grid"),
            ("tool-polygon", "dc-polygon"),
            ("tool-rect", "square"),
            ("tool-rect-cut", "dc-rect-cut"),
            ("tool-rect-grid", "dc-rect-grid"),
            ("tool-reflect", "flip-horizontal-2"),
            ("tool-rounded-rect", "dc-rounded-rect"),
            ("tool-scale", "scaling"),
            ("tool-scissors", "scissors"),
            ("tool-selection", "dc-selection"),
            ("tool-shape-builder", "dc-shape-builder"),
            ("tool-smooth", "dc-smooth"),
            ("tool-spiral", "tornado"),
            ("tool-star", "star"),
            ("tool-type", "type"),
            ("tool-zoom", "zoom-in"),
        ] {
            assert_eq!(crate::icons::tool_icon(alias), *catalogue);
            assert!(super::source(catalogue).is_some(), "{alias}");
        }
        for name in ["dc-free-transform", "dc-shear", "rotate-ccw", "dc-blend", "dc-width", "wand-sparkles", "dc-para-ltr", "dc-para-rtl"] {
            assert!(super::source(name).is_none(), "{name}: retain upstream");
        }
    }
}
